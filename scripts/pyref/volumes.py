import math
NAN=float('nan')
def isnan(v): return v!=v
CLOSE=[10.,11,12.5,11.5,13,14,13.5,15,16,15.5,17,18,17.5,19,20,19.5,21,22,21.5,23,24,23.5,25,26,25.5,27,28,27.5,29,30,29.5,31,32,31.5,33,34,33.5,35,36,35.5]
HIGH=[c+1 for c in CLOSE]; LOW=[c-1 for c in CLOSE]
VOL=[100.,150,120,180,140,160,130,170,150,190,110,140,160,120,180,150,130,170,140,160,
     100,150,120,180,140,160,130,170,150,190,110,140,160,120,180,150,130,170,140,160]
N=len(CLOSE)
def sma(src,l):
    out=[NAN]*len(src)
    for i in range(l-1,len(src)):
        w=src[i-l+1:i+1]
        out[i]=sum(w)/l if all(not isnan(v) for v in w) else NAN
    return out
def ema_sma(src,l):
    out=[NAN]*len(src); a=2/(l+1); seeded=False; prev=NAN
    for i,v in enumerate(src):
        if isnan(v):
            out[i]=prev if seeded else NAN; continue
        if not seeded:
            if i>=l-1:
                w=src[i-l+1:i+1]
                if all(not isnan(x) for x in w):
                    prev=sum(w)/l; seeded=True; out[i]=prev
            else: out[i]=NAN
        else:
            prev=a*v+(1-a)*prev; out[i]=prev
    return out
def ema_fb(src,l):
    out=[NAN]*len(src); a=2/(l+1); prev=NAN
    for i,v in enumerate(src):
        if isnan(v): out[i]=prev if not isnan(prev) else NAN; continue
        if isnan(prev): prev=v
        else: prev=a*v+(1-a)*prev
        out[i]=prev
    return out
def rma(src,l):
    out=[NAN]*len(src); a=1/l; seeded=False; prev=NAN
    for i,v in enumerate(src):
        if isnan(v): out[i]=prev if seeded else NAN; continue
        if not seeded:
            if i>=l-1:
                w=src[i-l+1:i+1]
                if all(not isnan(x) for x in w):
                    prev=sum(w)/l; seeded=True; out[i]=prev
            else: out[i]=NAN
        else:
            prev=a*v+(1-a)*prev; out[i]=prev
    return out
def arr(name,vals):
    parts=["f64::NAN" if isnan(v) else f"{v!r}" for v in vals]
    print(f"    const {name}: [f64; {len(vals)}] = [")
    line="        "
    for p in parts:
        if len(line)+len(p)+2>98: print(line.rstrip()); line="        "
        line+=p+", "
    print(line.rstrip()+"];\n")

# OBV
obv=[0.0]*N
for i in range(1,N):
    obv[i]=obv[i-1]+(VOL[i] if CLOSE[i]>CLOSE[i-1] else -VOL[i] if CLOSE[i]<CLOSE[i-1] else 0.0)
arr("OBV",obv)
# A/D + Chaikin Oscillator
ad=[0.0]*N
for i in range(N):
    r=HIGH[i]-LOW[i]
    mult=((CLOSE[i]-LOW[i])-(HIGH[i]-CLOSE[i]))/r if r!=0 else 0.0
    ad[i]=(ad[i-1] if i>0 else 0.0)+mult*VOL[i]
arr("AD",ad)
arr("CHAOS", ema_sma(ad,3) if False else [ (ema_sma(ad,3)[i]-ema_sma(ad,10)[i]) if not isnan(ema_sma(ad,3)[i]) and not isnan(ema_sma(ad,10)[i]) else NAN for i in range(N)])
# CMF / MFI
mfam=[NAN]*N; mf=[NAN]*N
for i in range(19,N):
    num=0.0; den=0.0
    ok=True
    for j in range(i-19,i+1):
        r=HIGH[j]-LOW[j]
        if r==0: ok=False
        num+=((CLOSE[j]-LOW[j])-(HIGH[j]-CLOSE[j]))/r*VOL[j] if r!=0 else 0.0
        den+=VOL[j]
    if ok and den!=0: mfam[i]=num/den
arr("MFAM",mfam)
tp=[(HIGH[i]+LOW[i]+CLOSE[i])/3 for i in range(N)]
mfiv=[tp[i]*VOL[i] for i in range(N)]
mfi=[NAN]*N
for i in range(14,N):
    pos=0.0; neg=0.0
    for j in range(i-13,i+1):
        if tp[j]>tp[j-1]: pos+=mfiv[j]
        elif tp[j]<tp[j-1]: neg+=mfiv[j]
    if neg!=0: mfi[i]=100-100/(1+pos/neg)
    elif pos>0: mfi[i]=100.0
arr("MFI",mfi)
# EOM
eom=[NAN]*N
for i in range(14,N):
    dist=[NAN]*N; ratio=[NAN]*N
    pass
eomv=[NAN]*N
for i in range(1,N):
    r=HIGH[i]-LOW[i]
    if r==0 or VOL[i]==0: continue
    dist=((HIGH[i]+LOW[i])-(HIGH[i-1]+LOW[i-1]))/2
    br=(VOL[i]/1.0)/r
    eomv[i]=dist/br
arr("EOMRAW",eomv)
sm=[NAN]*N
for i in range(14,N):
    w=eomv[i-13:i+1]
    if all(not isnan(x) for x in w): sm[i]=sum(w)/14
arr("EOM",sm)
# VPT
vpt=[0.0]*N
for i in range(1,N):
    vpt[i]=vpt[i-1]+(VOL[i]*math.log(CLOSE[i]/CLOSE[i-1]) if CLOSE[i-1]>0 and CLOSE[i]>0 else 0.0)
arr("VPT",vpt)
# PVI / DVI with EMAs
pvi=[0.0]*N; dvi=[0.0]*N
for i in range(1,N):
    if CLOSE[i]>CLOSE[i-1]: pvi[i]=pvi[i-1]+VOL[i]
    elif CLOSE[i]<CLOSE[i-1]: pvi[i]=pvi[i-1]-VOL[i]
    else: pvi[i]=pvi[i-1]
    if CLOSE[i]>CLOSE[i-1]: dvi[i]=dvi[i-1]
    elif CLOSE[i]<CLOSE[i-1]: dvi[i]=dvi[i-1]+VOL[i]
    else: dvi[i]=dvi[i-1]
arr("PVI",pvi); arr("DVI",dvi)
arr("PVI_MA",ema_fb(pvi,5)); arr("PVI_MA2",ema_fb(pvi,20)); arr("DVI_MA",ema_fb(dvi,5)); arr("DVI_MA2",ema_fb(dvi,20))
# Klinger
dma=[0.0]*N
for i in range(1,N): dma[i]=(HIGH[i]+LOW[i])-(HIGH[i-1]+LOW[i-1])
ev=ema_sma(VOL,34); ev2=ema_sma(VOL,55)
force=[NAN]*N
for i in range(N):
    if isnan(ev[i]) or isnan(ev2[i]): continue
    den=ev[i]+ev2[i]
    ratio=1.0 if den==0 else 1+ (ev[i]-ev2[i])/den
    f=VOL[i]*ratio
    force[i]= f if dma[i]>0 else -f if dma[i]<0 else 0.0
osc=[0.0]*N
for i in range(N): osc[i]= (osc[i-1] if i>0 else 0.0)+(force[i] if not isnan(force[i]) else 0.0)
kvo=[NAN]*N
e34=ema_sma(osc,34); e55=ema_sma(osc,55)
for i in range(N):
    if not isnan(e34[i]) and not isnan(e55[i]): kvo[i]=e34[i]-e55[i]
arr("KVO",kvo); arr("KVO_SIG",ema_sma(kvo,9))
# Accumulation/Distribution oscillator done; AC
hl2=[(HIGH[i]+LOW[i])/2 for i in range(N)]
ao=[ (sma(hl2,5)[i]-sma(hl2,34)[i]) if not isnan(sma(hl2,5)[i]) and not isnan(sma(hl2,34)[i]) else NAN for i in range(N)]
arr("AC",[ao[i]-sma(ao,5)[i] if not isnan(ao[i]) and not isnan(sma(ao,5)[i]) else NAN for i in range(N)])
# VR
vr=[NAN]*N
for i in range(26,N):
    up=dn=un=0.0
    for j in range(i-25,i+1):
        if CLOSE[j]>CLOSE[j-1]: up+=VOL[j]
        elif CLOSE[j]<CLOSE[j-1]: dn+=VOL[j]
        else: un+=VOL[j]
    d=dn+0.5*un
    if d!=0: vr[i]=100*(up+0.5*un)/d
arr("VR",vr)
# Elder Force
efi_src=[VOL[i]*(1 if CLOSE[i]>CLOSE[i-1] else -1 if CLOSE[i]<CLOSE[i-1] else 0) for i in range(1,N)]
efi=[NAN]+ema_sma(efi_src,13)
arr("EFI",efi)
print("N =",N)
