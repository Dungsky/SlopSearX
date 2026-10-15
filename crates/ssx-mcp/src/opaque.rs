//! Opaque descriptor boundary: rmcp's Value model cannot represent every JSON number.
//! RawValue retains schema/metadata tokens without a global arbitrary_precision feature.
use crate::{Registry, Result};
use serde::Deserialize;
use serde_json::{Value, json, value::RawValue};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub type Catalogs = BTreeMap<String, Box<RawValue>>;
/// JSON-lines framing removes only insignificant ASCII whitespace outside strings.
/// Validation is performed by RawValue first; number and string tokens stay untouched.
pub fn compact(raw: &str) -> Result<String> {
    let _: Box<RawValue> = serde_json::from_str(raw)?;
    let mut output = String::with_capacity(raw.len());
    let mut quoted = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if quoted {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else if ch == '"' {
            quoted = true;
            output.push(ch);
        } else if !matches!(ch, ' ' | '\t' | '\r' | '\n') {
            output.push(ch);
        }
    }
    Ok(output)
}
#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    id: Option<Box<RawValue>>,
    #[serde(default)]
    method: Option<String>,
}
/// Source MCP 2.3 lifecycle: initialize response grants requests; the notification
/// is advisory. Keep rmcp's dispatcher initialized without changing wire ordering.
#[derive(Default)]
pub struct Lifecycle {
    accepted: bool,
    initializing: HashSet<String>,
}
impl Lifecycle {
    /// None means forward; Some(None) means consume notification; Some(Some(reply))
    /// means return the source-compatible error without advancing the SDK.
    pub fn request(&mut self, line: &str) -> Result<Option<Option<String>>> {
        let envelope: Envelope = serde_json::from_str(line)?;
        let Some(method) = envelope.method.as_deref() else {
            return Ok(None);
        };
        if method == "notifications/initialized" {
            return Ok(Some(None));
        }
        let Some(id) = envelope.id else {
            return Ok(None);
        };
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
        let initialize_valid = method == "initialize"
            && fields.get("params").is_some_and(|p| {
                serde_json::from_str::<rmcp::model::InitializeRequestParams>(p.get()).is_ok()
            });
        if initialize_valid {
            self.initializing.insert(key(&id)?);
            return Ok(None);
        }
        if method == "ping" || (self.accepted && method != "initialize") {
            return Ok(None);
        }
        let known = matches!(
            method,
            "initialize"
                | "tools/list"
                | "tools/call"
                | "resources/list"
                | "resources/templates/list"
                | "resources/read"
                | "prompts/list"
                | "prompts/get"
                | "logging/setLevel"
        );
        let error = if known {
            json!({"code":-32602,"message":"Invalid request parameters","data":""})
        } else {
            json!({"code":-32601,"message":"Method not found","data":method})
        };
        Ok(Some(Some(format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{}}}",
            id.get(),
            error
        ))))
    }
    pub fn remap(&mut self, original: &str, rewritten: &str) -> Result<()> {
        if original != rewritten {
            let original: Envelope = serde_json::from_str(original)?;
            let rewritten: Envelope = serde_json::from_str(rewritten)?;
            if let (Some(old), Some(new)) = (original.id, rewritten.id)
                && self.initializing.remove(&key(&old)?)
            {
                self.initializing.insert(key(&new)?);
            }
        }
        Ok(())
    }
    pub fn response(&mut self, line: &str) -> Result<bool> {
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
        if let Some(id) = fields.get("id")
            && self.initializing.remove(&key(id)?)
            && fields.contains_key("result")
            && !self.accepted
        {
            self.accepted = true;
            return Ok(true);
        }
        Ok(false)
    }
}
pub const INITIALIZED: &[u8] = b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
/// Match only a successful SDK response to a registered list request. Protocol errors and
/// lifecycle validation remain the SDK's authority; only the opaque result payload changes.
#[derive(Default)]
pub struct Overlay {
    pending: HashMap<String, String>,
}
pub fn key(id: &RawValue) -> Result<String> {
    let raw = id.get();
    if raw.starts_with('"') {
        return Ok(format!("string:{}", serde_json::from_str::<String>(raw)?));
    }
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err("MCP request id must be a string or integer".into());
    }
    Ok(format!("integer:{}", if raw == "-0" { "0" } else { raw }))
}
/// Validate SDK envelope shape using disposable aliases, including cancellation IDs.
/// Real session alias ownership stays with WireState in the transport pump.
pub fn sdk_validation(line: &str) -> Result<String> {
    let mut wire = WireState::default();
    let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
    if fields
        .get("method")
        .is_some_and(|m| m.get() == "\"notifications/cancelled\"")
        && let Some(params) = fields.get("params")
    {
        let params: BTreeMap<String, Box<RawValue>> = serde_json::from_str(params.get())?;
        if let Some(id) = params.get("requestId") {
            wire.request(&format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"tools/list\"}}",
                id.get()
            ))?;
        }
    }
    wire.request(line)
}
pub struct Frames<R> {
    reader: BufReader<R>,
    partial: Vec<u8>,
}
impl<R: tokio::io::AsyncRead + Unpin> Frames<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            partial: Vec::new(),
        }
    }
    pub async fn next(&mut self) -> Result<Option<String>> {
        loop {
            let bytes = self.reader.fill_buf().await?;
            if bytes.is_empty() {
                return if self.partial.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(String::from_utf8(std::mem::take(&mut self.partial))?))
                };
            }
            let end = bytes
                .iter()
                .position(|b| *b == b'\n')
                .map(|p| p + 1)
                .unwrap_or(bytes.len());
            if self.partial.len() + end > 4 * 1024 * 1024 {
                return Err("MCP frame exceeds 4 MiB".into());
            }
            let complete = bytes[end - 1] == b'\n';
            self.partial.extend_from_slice(&bytes[..end]);
            self.reader.consume(end);
            if complete {
                self.partial.pop();
                if self.partial.last() == Some(&b'\r') {
                    self.partial.pop();
                }
                return Ok(Some(String::from_utf8(std::mem::take(&mut self.partial))?));
            }
        }
    }
}
#[derive(Default)]
pub struct WireState {
    requests: HashMap<String, String>,
    replies: HashMap<String, String>,
    synthetic: HashMap<String, String>,
    aliases: HashMap<String, Box<RawValue>>,
    reverse: HashMap<String, Box<RawValue>>,
    active: HashSet<String>,
}
/// The synthetic echo has an unbounded integer schema. Validate raw integer
/// tokens before Value decoding, give the SDK exact safe placeholders, and keep
/// the real echo payload for replacement only after handler success. This is a
/// qualification fixture boundary, not a generic domain JSON interpreter.
fn synthetic_integer_call(line: &str) -> Result<Option<(String, String)>> {
    let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
    if fields
        .get("method")
        .is_none_or(|v| v.get() != "\"tools/call\"")
    {
        return Ok(None);
    }
    let Some(id) = fields.get("id").cloned() else {
        return Ok(None);
    };
    let Some(params) = fields.get("params") else {
        return Ok(None);
    };
    let mut params: BTreeMap<String, Box<RawValue>> = match serde_json::from_str(params.get()) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    if !params
        .get("name")
        .is_some_and(|v| ["\"t01_echo\"", "\"t01_large\""].contains(&v.get()))
    {
        return Ok(None);
    }
    let Some(args) = params.get("arguments") else {
        return Ok(None);
    };
    let mut args: BTreeMap<String, Box<RawValue>> = match serde_json::from_str(args.get()) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let Some(payload) = args.get("payload").cloned() else {
        return Ok(None);
    };
    let mut safe_payload: BTreeMap<String, Box<RawValue>> =
        match serde_json::from_str(payload.get()) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
    let Some(items) = safe_payload.get("items") else {
        return Ok(None);
    };
    let mut items: Vec<Box<RawValue>> = match serde_json::from_str(items.get()) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let mut changed = false;
    for item in &mut items {
        let raw = item.get();
        let digits = raw.strip_prefix('-').unwrap_or(raw);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Ok(None);
        }
        if raw.parse::<i64>().is_err() && raw.parse::<u64>().is_err() {
            *item = RawValue::from_string("0".into())?;
            changed = true;
        }
    }
    if !changed {
        return Ok(None);
    }
    let note = args.get("note").map(|v| v.get()).unwrap_or("null");
    let echoed = compact(&format!(
        "{{\"payload\":{},\"note\":{note}}}",
        payload.get()
    ))?;
    let reply = compact(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":{}}}],\"structuredContent\":{echoed},\"isError\":false}}}}",
        id.get(),
        serde_json::to_string(&echoed)?
    ))?;
    safe_payload.insert(
        "items".into(),
        RawValue::from_string(serde_json::to_string(&items)?)?,
    );
    args.insert(
        "payload".into(),
        RawValue::from_string(serde_json::to_string(&safe_payload)?)?,
    );
    params.insert(
        "arguments".into(),
        RawValue::from_string(serde_json::to_string(&args)?)?,
    );
    fields.insert(
        "params".into(),
        RawValue::from_string(serde_json::to_string(&params)?)?,
    );
    Ok(Some((compact(&serde_json::to_string(&fields)?)?, reply)))
}
impl WireState {
    pub fn request(&mut self, line: &str) -> Result<String> {
        let envelope: Envelope = serde_json::from_str(line)?;
        if envelope.method.as_deref() == Some("notifications/cancelled") {
            let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
            if let Some(params) = fields.get_mut("params") {
                let mut values: BTreeMap<String, Box<RawValue>> =
                    serde_json::from_str(params.get())?;
                if let Some(id) = values.get("requestId")
                    && let Some(alias) = self.reverse.get(&key(id)?)
                {
                    values.insert("requestId".into(), alias.clone());
                    *params = RawValue::from_string(serde_json::to_string(&values)?)?;
                    return compact(&serde_json::to_string(&fields)?);
                }
            }
        }
        if let (Some(id), Some(_)) = (envelope.id, envelope.method) {
            let original = key(&id)?;
            if self.active.contains(&original) {
                return Err("duplicate MCP request id".into());
            }
            if self.active.len() >= 32 {
                return Err("MCP peer exceeds 32 concurrent requests".into());
            }
            let mut alias_binding = None;
            let mut rewritten = line.to_owned();
            let mut internal = original.clone();
            if !id.get().starts_with('"') && id.get().parse::<i64>().is_err() {
                let alias = RawValue::from_string(serde_json::to_string(&format!(
                    "ssx-{}",
                    uuid::Uuid::new_v4()
                ))?)?;
                internal = key(&alias)?;
                alias_binding = Some((id, alias.clone()));
                let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
                fields.insert("id".into(), alias);
                rewritten = compact(&serde_json::to_string(&fields)?)?;
            }
            let synthetic = synthetic_integer_call(&rewritten)?;
            // Admission is transactional: nothing above mutates session bookkeeping.
            self.active.insert(original.clone());
            if let Some((id, alias)) = alias_binding {
                self.aliases.insert(internal.clone(), id);
                self.reverse.insert(original, alias);
            }
            if let Some((safe, reply)) = synthetic {
                self.synthetic.insert(internal.clone(), reply);
                rewritten = safe;
            }
            self.requests.insert(internal, line.into());
            return Ok(rewritten);
        }
        Ok(line.into())
    }
    pub fn abandon(&mut self, request: &str) {
        if let Ok(envelope) = serde_json::from_str::<Envelope>(request)
            && let Some(id) = envelope.id
        {
            let _ = self.response(&format!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":-32603,\"message\":\"cancelled\"}}}}",id.get()));
        }
    }
    pub fn response(&mut self, line: &str) -> Result<String> {
        let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
        let Some(id) = fields.get("id") else {
            return Ok(line.into());
        };
        let internal = key(id)?;
        if !self.requests.contains_key(&internal) {
            return Ok(line.into());
        }
        self.requests.remove(&internal);
        let synthetic = self.synthetic.remove(&internal);
        let success = fields.get("result").is_some_and(|result| {
            serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(result.get())
                .ok()
                .is_some_and(|fields| {
                    fields
                        .get("isError")
                        .is_none_or(|value| value.get() != "true")
                })
        }) && !fields.contains_key("error");
        let mut result = self
            .replies
            .remove(&internal)
            .or(if success { synthetic } else { None })
            .unwrap_or_else(|| line.into());
        if let Some(original) = self.aliases.remove(&internal) {
            self.reverse.remove(&key(&original)?);
            self.active.remove(&key(&original)?);
            fields = serde_json::from_str(&result)?;
            fields.insert("id".into(), original);
            result = compact(&serde_json::to_string(&fields)?)?;
        } else {
            self.active.remove(&internal);
        }
        Ok(result)
    }
}
impl Overlay {
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
    pub fn request(&mut self, catalogs: &Catalogs, request: &str) -> Result<()> {
        let envelope: Envelope = serde_json::from_str(request)?;
        if let (Some(id), Some(method)) = (envelope.id, envelope.method)
            && let Some(result) = catalogs.get(&method)
        {
            self.pending.insert(
                key(&id)?,
                compact(&format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}",
                    id.get(),
                    result.get()
                ))?,
            );
        }
        Ok(())
    }
    pub fn response(&mut self, response: &str) -> Result<String> {
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(response)?;
        if let Some(id) = fields.get("id")
            && let Some(replacement) = self.pending.remove(&key(id)?)
            && fields.contains_key("result")
            && !fields.contains_key("error")
        {
            return Ok(replacement);
        }
        Ok(response.into())
    }
}
/// Streaming SSE frame rewrite, preserving event IDs, retry fields and comments.
pub fn overlay_event(overlay: &mut Overlay, raw: &str) -> Result<String> {
    rewrite_event(raw, |data| overlay.response(data))
}
pub fn wire_event(wire: &mut WireState, raw: &str) -> Result<String> {
    rewrite_event(raw, |data| wire.response(data))
}
fn rewrite_event(raw: &str, mut rewrite: impl FnMut(&str) -> Result<String>) -> Result<String> {
    let mut data = String::new();
    let mut fields = Vec::new();
    for line in raw.lines() {
        if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.strip_prefix(' ').unwrap_or(value));
        } else if !line.is_empty() {
            fields.push(line.to_owned());
        }
    }
    if data.is_empty() {
        return Ok(raw.into());
    }
    let rewritten = rewrite(&data)?;
    if rewritten == data {
        return Ok(raw.into());
    }
    fields.push(format!("data: {}", compact(&rewritten)?));
    Ok(format!("{}\n\n", fields.join("\n")))
}
/// Stdio framing adapter around the same real rmcp service. Never interprets catalog numbers.
pub async fn stdio(registry: Registry) -> Result<()> {
    let catalogs = registry.opaque.clone();
    stdio_handler(registry, catalogs, Default::default()).await
}
async fn stdio_handler<H: rmcp::ServerHandler>(
    handler: H,
    catalogs: std::sync::Arc<Catalogs>,
    wire: std::sync::Arc<std::sync::Mutex<WireState>>,
) -> Result<()> {
    use rmcp::ServiceExt;
    let (server, io) = tokio::io::duplex(65536);
    let (read, mut write) = tokio::io::split(io);
    let serving = async {
        handler.serve(server).await?.waiting().await?;
        Ok::<(), crate::Error>(())
    };
    let pump = async {
        let mut input = Frames::new(tokio::io::stdin());
        let mut output = Frames::new(read);
        let mut stdout = tokio::io::stdout();
        let mut overlay = Overlay::default();
        let mut lifecycle = Lifecycle::default();
        loop {
            tokio::select! {
                line=input.next()=>{let Some(line)=line? else{break;};if let Some(reply)=lifecycle.request(&line)?{if let Some(reply)=reply{stdout.write_all(reply.as_bytes()).await?;stdout.write_all(b"\n").await?;stdout.flush().await?;}continue;}let rewritten=wire.lock().unwrap().request(&line)?;lifecycle.remap(&line,&rewritten)?;let line=rewritten;overlay.request(&catalogs,&line)?;write.write_all(line.as_bytes()).await?;write.write_all(b"\n").await?;},
                line=output.next()=>{let Some(line)=line? else{break;};if lifecycle.response(&line)?{write.write_all(INITIALIZED).await?;}let line=overlay.response(&line)?;let line=wire.lock().unwrap().response(&line)?;stdout.write_all(compact(&line)?.as_bytes()).await?;stdout.write_all(b"\n").await?;stdout.flush().await?;}
            }
        }
        Ok::<(), crate::Error>(())
    };
    tokio::select! {r=serving=>r,r=pump=>r}
}
fn validate_reply(fields: &BTreeMap<String, Box<RawValue>>, expected: Option<&str>) -> Result<()> {
    if fields.get("jsonrpc").map(|v| v.get()) != Some("\"2.0\"")
        || fields.contains_key("result") == fields.contains_key("error")
    {
        return Err("invalid remote JSON-RPC response".into());
    }
    let actual = fields.get("id").map(|v| key(v)).transpose()?;
    if actual.as_deref() != expected {
        return Err("remote JSON-RPC response id mismatch".into());
    }
    Ok(())
}
#[derive(Clone)]
pub struct RawGateway {
    http: reqwest::Client,
    url: String,
    token: Option<String>,
    pub catalogs: Catalogs,
    deadline: Duration,
    session: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    initialize_result: Option<Box<RawValue>>,
    protocol: String,
}
impl RawGateway {
    async fn exchange(&self, body: &str) -> Result<Option<String>> {
        let mut request = self
            .http
            .post(&self.url)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .header("mcp-protocol-version", self.protocol.as_str())
            .body(body.to_owned());
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(session) = self.session.lock().unwrap().clone() {
            request = request.header("mcp-session-id", session);
        }
        let mut response = tokio::time::timeout(self.deadline, request.send()).await??;
        if let Some(id) = response.headers().get("mcp-session-id") {
            let id = id.to_str()?.to_owned();
            let mut current = self.session.lock().unwrap();
            if current.as_ref().is_some_and(|old| old != &id) {
                return Err("remote MCP session identity changed".into());
            }
            *current = Some(id);
        }
        if response.status() == 202 || response.status() == 204 {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!("remote MCP HTTP status {}", response.status()).into());
        }
        let sse = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/event-stream"));
        let request: Envelope = serde_json::from_str(body)?;
        let expected = request.id.as_ref().map(|v| key(v)).transpose()?;
        let read = async {
            let mut bytes = Vec::new();
            let mut data = Vec::new();
            let mut total = 0usize;
            let mut events = 0usize;
            while let Some(chunk) = response.chunk().await? {
                total = total.saturating_add(chunk.len());
                if total > 4 * 1024 * 1024 {
                    return Err("remote MCP body exceeds 4 MiB".into());
                }
                bytes.extend_from_slice(&chunk);
                if sse {
                    while let Some(end) = bytes.iter().position(|b| *b == b'\n') {
                        let mut line: Vec<u8> = bytes.drain(..=end).collect();
                        line.pop();
                        if line.last() == Some(&b'\r') {
                            line.pop();
                        }
                        if line.is_empty() {
                            if !data.is_empty() {
                                events += 1;
                                if events > 128 {
                                    return Err("MCP SSE exceeds 128 events".into());
                                }
                                let message = String::from_utf8(std::mem::take(&mut data))?;
                                let fields: BTreeMap<String, Box<RawValue>> =
                                    serde_json::from_str(&message)?;
                                if fields.contains_key("result") || fields.contains_key("error") {
                                    validate_reply(&fields, expected.as_deref())?;
                                    return Ok(Some(message));
                                }
                            }
                        } else if let Some(value) = line.strip_prefix(b"data:") {
                            let value = value.strip_prefix(b" ").unwrap_or(value);
                            if !data.is_empty() {
                                data.push(b'\n');
                            }
                            data.extend_from_slice(value);
                        }
                    }
                }
            }
            if sse {
                return Err("MCP SSE ended without correlated response".into());
            }
            let body = String::from_utf8(bytes)?;
            let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&body)?;
            validate_reply(&fields, expected.as_deref())?;
            Ok(Some(body))
        };
        tokio::time::timeout(self.deadline, read).await?
    }
    pub async fn connect(url: String, token: Option<String>, deadline: Duration) -> Result<Self> {
        let parsed = reqwest::Url::parse(&url)?;
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err("invalid MCP remote URL".into());
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut gateway = Self {
            http,
            url,
            token,
            catalogs: BTreeMap::new(),
            deadline,
            session: Default::default(),
            initialize_result: None,
            protocol: "2025-11-25".into(),
        };
        let startup=tokio::time::timeout(deadline,async {
            let initialize=gateway.exchange(&json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"ssx-native-proxy","version":"0.1.0"}}}).to_string()).await?.ok_or("remote initialize had no result")?;
            let mut fields:BTreeMap<String,Box<RawValue>>=serde_json::from_str(&initialize)?;if !fields.contains_key("result") || fields.contains_key("error"){return Err("remote initialize failed".into());}
            gateway.initialize_result=fields.remove("result");
            let initialize_fields:BTreeMap<String,Box<RawValue>>=serde_json::from_str(gateway.initialize_result.as_ref().ok_or("initialize result")?.get())?;
            let version:String=serde_json::from_str(initialize_fields.get("protocolVersion").ok_or("remote protocol version missing")?.get())?;
            if !["2024-11-05","2025-03-26","2025-06-18","2025-11-25"].contains(&version.as_str()){return Err("unsupported remote initialize protocol version".into());}
            gateway.protocol=version;
            gateway.exchange(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).await?;
            for (method,field) in [("tools/list","tools"),("resources/list","resources"),("prompts/list","prompts"),("resources/templates/list","resourceTemplates")] {
                let mut items:Vec<Box<RawValue>>=Vec::new();let mut names=HashSet::new();let mut cursors=HashSet::new();let mut cursor=None;let mut complete=false;
                for page in 0..128 {
                    let params=cursor.as_ref().map(|c|json!({"cursor":c})).unwrap_or(json!({}));
                    let body=gateway.exchange(&json!({"jsonrpc":"2.0","id":page+1,"method":method,"params":params}).to_string()).await?.ok_or("empty catalog response")?;
                    let envelope:BTreeMap<String,Box<RawValue>>=serde_json::from_str(&body)?;
                    if envelope.contains_key("error"){return Err("remote catalog error".into());}
                    let result:BTreeMap<String,Box<RawValue>>=serde_json::from_str(envelope.get("result").ok_or("catalog result missing")?.get())?;
                    let rows:Vec<Box<RawValue>>=serde_json::from_str(result.get(field).ok_or("catalog entries missing")?.get())?;
                    for row in rows {let data:BTreeMap<String,Box<RawValue>>=serde_json::from_str(row.get())?;let identity=crate::gateway::catalog_identity(method)?;let name:String=serde_json::from_str(data.get(identity).ok_or("catalog identity missing")?.get())?;if !names.insert(name){return Err("duplicate catalog name".into());}items.push(row);if items.len()>4096{return Err("catalog exceeds 4096 items".into());}}
                    let next:Option<String>=result.get("nextCursor").map(|c|serde_json::from_str(c.get())).transpose()?.flatten();
                    let Some(next)=next else{complete=true;break;};if !cursors.insert(next.clone()){return Err("repeated catalog cursor".into());}cursor=Some(next);
                }
                if !complete{return Err("catalog exceeds 128 pages".into());}
                let raw=RawValue::from_string(format!("{{{}:{}}}",serde_json::to_string(field)?,serde_json::to_string(&items)?))?;gateway.catalogs.insert(method.into(),raw);
            }
            Ok::<(),crate::Error>(())
        }).await.map_err(|e|->crate::Error{e.into()}).and_then(|v|v);
        if let Err(error) = startup {
            let _ = gateway.close().await;
            return Err(error);
        }
        Ok(gateway)
    }
    pub async fn stdio(self) -> Result<()> {
        let wire: std::sync::Arc<std::sync::Mutex<WireState>> = Default::default();
        let handler = RawForwarder {
            gateway: self.clone(),
            wire: wire.clone(),
        };
        let result = stdio_handler(handler, std::sync::Arc::new(self.catalogs.clone()), wire).await;
        self.close().await?;
        result
    }
    pub async fn close(&self) -> Result<()> {
        let session = self.session.lock().unwrap().take();
        if let Some(session) = session {
            let mut request = self
                .http
                .delete(&self.url)
                .header("mcp-session-id", session)
                .header("mcp-protocol-version", self.protocol.as_str());
            if let Some(token) = &self.token {
                request = request.bearer_auth(token);
            }
            let response = tokio::time::timeout(self.deadline, request.send()).await??;
            if !response.status().is_success()
                && response.status() != 404
                && response.status() != 405
            {
                return Err("remote session cleanup failed".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct RawForwarder {
    gateway: RawGateway,
    wire: std::sync::Arc<std::sync::Mutex<WireState>>,
}
impl RawForwarder {
    fn decode<T: serde::de::DeserializeOwned>(
        value: Value,
    ) -> std::result::Result<T, rmcp::ErrorData> {
        serde_json::from_value(value)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
    }
    async fn forward<T: serde::de::DeserializeOwned>(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<T, rmcp::ErrorData> {
        let id = key(&RawValue::from_string(
            serde_json::to_string(&context.id)
                .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?,
        )
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?)
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        let request = self
            .wire
            .lock()
            .unwrap()
            .requests
            .get(&id)
            .cloned()
            .ok_or_else(|| {
                rmcp::ErrorData::internal_error("original wire request missing", None)
            })?;
        // Retain the pending HTTP future until cancellation is delivered. Dropping
        // it first tears down the remote request's alias lease before the cancel
        // notification can correlate its original arbitrary-size ID.
        let exchange = self.gateway.exchange(&request);
        tokio::pin!(exchange);
        let response = tokio::select! {
            r=&mut exchange=>r.map_err(|_|rmcp::ErrorData::internal_error("remote transport unavailable",None))?.ok_or_else(||rmcp::ErrorData::internal_error("remote RPC response missing",None))?,
            _=context.ct.cancelled()=>{
                let fields:BTreeMap<String,Box<RawValue>>=serde_json::from_str(&request).map_err(|e|rmcp::ErrorData::internal_error(e.to_string(),None))?;
                if let Some(id)=fields.get("id"){let notification=format!("{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{{\"requestId\":{}}}}}",id.get());let _=self.gateway.exchange(&notification).await;}
                return Err(rmcp::ErrorData::internal_error("cancelled",None));
            }
        };
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&response)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        let decoded = if let Some(error) = fields.get("error") {
            Err(serde_json::from_str::<rmcp::ErrorData>(error.get())
                .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?)
        } else {
            Ok(serde_json::from_str::<T>(
                fields
                    .get("result")
                    .ok_or_else(|| rmcp::ErrorData::internal_error("remote result missing", None))?
                    .get(),
            )
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?)
        };
        self.wire.lock().unwrap().replies.insert(id, response);
        decoded
    }
}
impl rmcp::ServerHandler for RawForwarder {
    fn get_info(&self) -> rmcp::model::ServerConfig {
        self.gateway
            .initialize_result
            .as_ref()
            .and_then(|v| serde_json::from_str(v.get()).ok())
            .unwrap_or_default()
    }
    async fn list_tools(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        Self::decode(
            serde_json::from_str(self.gateway.catalogs["tools/list"].get()).unwrap_or_default(),
        )
    }
    async fn list_resources(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ListResourcesResult, rmcp::ErrorData> {
        Self::decode(
            serde_json::from_str(self.gateway.catalogs["resources/list"].get()).unwrap_or_default(),
        )
    }
    async fn list_prompts(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ListPromptsResult, rmcp::ErrorData> {
        Self::decode(
            serde_json::from_str(self.gateway.catalogs["prompts/list"].get()).unwrap_or_default(),
        )
    }
    async fn list_resource_templates(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ListResourceTemplatesResult, rmcp::ErrorData> {
        Self::decode(
            serde_json::from_str(self.gateway.catalogs["resources/templates/list"].get())
                .unwrap_or_default(),
        )
    }
    async fn call_tool(
        &self,
        _: rmcp::model::CallToolRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        self.forward::<rmcp::model::CallToolResult>(c)
            .await
            .map(Into::into)
    }
    async fn read_resource(
        &self,
        _: rmcp::model::ReadResourceRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ReadResourceResponse, rmcp::ErrorData> {
        self.forward::<rmcp::model::ReadResourceResult>(c)
            .await
            .map(Into::into)
    }
    async fn get_prompt(
        &self,
        _: rmcp::model::GetPromptRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::GetPromptResponse, rmcp::ErrorData> {
        self.forward::<rmcp::model::GetPromptResult>(c)
            .await
            .map(Into::into)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_requests_do_not_leak_admission_capacity() -> Result<()> {
        let mut wire = WireState::default();
        let request = |id: u128| format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/list"}}"#);
        let base = u128::from(u64::MAX) + 1;
        let mut admitted = Vec::new();
        for id in base..base + 32 {
            admitted.push(wire.request(&request(id))?);
        }
        assert!(
            wire.request(&request(base))
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
        for id in base + 32..base + 1032 {
            assert!(wire.request(&request(id)).is_err());
        }
        assert_eq!(wire.active.len(), 32);
        assert_eq!(wire.requests.len(), 32);
        assert_eq!(wire.aliases.len(), 32);
        for request in admitted {
            wire.abandon(&request);
        }
        assert!(wire.active.is_empty());
        assert!(wire.requests.is_empty());
        assert!(wire.aliases.is_empty());
        assert!(wire.reverse.is_empty());
        for id in base..base + 32 {
            assert!(wire.request(&request(id)).is_ok());
        }
        Ok(())
    }
    #[test]
    fn adjacent_large_request_ids_do_not_alias_and_sdk_ids_restore_exactly() -> Result<()> {
        let mut wire = WireState::default();
        let mut aliases = Vec::new();
        for id in ["18446744073709551616", "18446744073709551617"] {
            let request = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"tools/list\"}}");
            let rewritten = wire.request(&request)?;
            let envelope: Envelope = serde_json::from_str(&rewritten)?;
            let alias = envelope.id.unwrap();
            assert!(alias.get().starts_with('"'));
            aliases.push((id, alias));
        }
        assert_ne!(key(&aliases[0].1)?, key(&aliases[1].1)?);
        for (original, alias) in aliases.into_iter().rev() {
            let response = format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{}}}}",
                alias.get()
            );
            let restored = wire.response(&response)?;
            let envelope: Envelope = serde_json::from_str(&restored)?;
            assert_eq!(envelope.id.unwrap().get(), original);
        }
        assert!(wire.active.is_empty());
        assert!(key(&RawValue::from_string("1.5".into())?).is_err());
        Ok(())
    }
    #[tokio::test]
    async fn newline_free_frame_is_rejected_before_eof_or_unbounded_allocation() -> Result<()> {
        let (mut writer, reader) = tokio::io::duplex(16384);
        let task = tokio::spawn(async move {
            let block = vec![b'x'; 16384];
            for _ in 0..257 {
                writer.write_all(&block).await?;
            }
            std::future::pending::<std::io::Result<()>>().await
        });
        let mut frames = Frames::new(reader);
        let result = tokio::time::timeout(Duration::from_secs(2), frames.next()).await?;
        assert!(result.is_err());
        assert!(frames.partial.len() <= 4 * 1024 * 1024);
        task.abort();
        let _ = task.await;
        Ok(())
    }
    #[test]
    fn compaction_preserves_strings_numbers_and_reserved_objects() -> Result<()> {
        let source = r#" { "x" : 18446744073709551617, "text": " spaces \" retained ", "y":{"$serde_json::private::Number":"123"} } "#;
        let out = compact(source)?;
        assert!(out.contains("18446744073709551617"));
        assert!(out.contains(r#"" spaces \" retained ""#));
        assert!(out.contains("$serde_json::private::Number"));
        Ok(())
    }
}
