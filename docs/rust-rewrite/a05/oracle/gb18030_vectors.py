import argparse,struct,zlib
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);args=p.parse_args();compress=zlib.compressobj(9);chunks=[];count=0
for a in range(0x81,0xff):
 for b in range(0x30,0x3a):
  for c in range(0x81,0xff):
   for d in range(0x30,0x3a):
    points=list(map(ord,bytes([a,b,c,d]).decode('gb18030','replace')));assert 1<=len(points)<=4
    row=struct.pack('>BIIII',len(points),*(points+[0xffffffff]*4)[:4]);part=compress.compress(row)
    if part:chunks.append(part)
    count+=1
chunks.append(compress.flush());args.output.write_bytes(b''.join(chunks));print(count,args.output.stat().st_size)
