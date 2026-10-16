"""Actual pinned codec observations for every generated name and range control."""
import argparse,json,re
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--names',type=Path,required=True);p.add_argument('--ucd',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
names=re.findall(r'\(\s*"([^"]+)",\s*\d+\s*\)' ,args.names.read_text(),re.M)
assert len(names)==51662
for line in (args.ucd/'NamedSequences.txt').read_text().splitlines():
 if line and not line.startswith('#'):names.append(line.split(';')[0].strip())
names+=['HANGUL SYLLABLE GAX','HANGUL SYLLABLE ','CJK UNIFIED IDEOGRAPH-04E00','CJK UNIFIED IDEOGRAPH-4e00','CJK UNIFIED IDEOGRAPH-4E00 ','SNOWMAN\0',' SNOWMAN','SNOWMAN ']
for lo,hi,prefix in [(0x3400,0x4dbf,'CJK UNIFIED IDEOGRAPH-'),(0x4e00,0x9fff,'CJK UNIFIED IDEOGRAPH-'),(0x17000,0x187f7,'TANGUT IDEOGRAPH-'),(0x18d00,0x18d08,'TANGUT IDEOGRAPH-'),(0x20000,0x2a6df,'CJK UNIFIED IDEOGRAPH-'),(0x2a700,0x2b739,'CJK UNIFIED IDEOGRAPH-'),(0x2b740,0x2b81d,'CJK UNIFIED IDEOGRAPH-'),(0x2b820,0x2cea1,'CJK UNIFIED IDEOGRAPH-'),(0x2ceb0,0x2ebe0,'CJK UNIFIED IDEOGRAPH-'),(0x2ebf0,0x2ee5d,'CJK UNIFIED IDEOGRAPH-'),(0x30000,0x3134a,'CJK UNIFIED IDEOGRAPH-'),(0x31350,0x323af,'CJK UNIFIED IDEOGRAPH-')]:
 for cp in [lo-1,lo,hi,hi+1]:names.append(prefix+f'{cp:04X}')
# Lowercase controls exercise the actual codec, not a normalized expectation.
names=list(dict.fromkeys(names+[n.lower() for n in names]))
rows=[]
for name in names:
 body=b'\\N{'+name.encode('utf8')+b'}';rows.append([name,list(map(ord,body.decode('unicode_escape','replace')))])
args.output.write_text(json.dumps(rows,ensure_ascii=True,separators=(',',':'))+'\n');print(len(rows))
