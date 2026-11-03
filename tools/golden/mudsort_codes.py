import re,sys
src=open(sys.argv[1],encoding='utf-8-sig').read()
enums=[]
for m in re.finditer(r'public enum (MS\w+ValueKey)\s*\{(.*?)\n\s*\}',src,re.S):
    kind=m.group(1)
    body=re.sub(r'//[^\n]*','',m.group(2))
    for e in re.finditer(r'(\w+)\s*=\s*([0-9xA-Fa-f]+)',body):
        enums.append((kind,e.group(1),int(e.group(2),0)))
# C# Enum.GetValues sorts by unsigned value
order={'MSStringValueKey':0,'MSLongValueKey':1,'MSDoubleValueKey':2,'MSBoolValueKey':3}
groups={}
for k,n,v in enums: groups.setdefault(k,[]).append((v & 0xffffffff,n))
codes=['OC','TR','BW','BE','BD','BA','BP','BM','TM','TS']
out=[]
for k in sorted(groups,key=order.get):
    for v,name in sorted(groups[k]):
        code=''
        for c in name:
            if c.isupper() and c.isalpha():
                code+=c
                if len(code)==2:
                    if code in codes: code=code[:1]
                    else: break
        if len(code)<2:
            for c in name:
                if c in 'bcdfghjklmnpqrstvwxyz': code+=c
                if len(code)==2:
                    if code.upper() in codes: code=code[:1]
                    else: break
            code=code.upper()
        if len(code)<2: code=name[:2].upper()
        if code not in codes:
            codes.append(code); out.append((code,k[2:-8],name,v))
        else: out.append(('DUP',k,name,v))
for o in out: print(*o)
