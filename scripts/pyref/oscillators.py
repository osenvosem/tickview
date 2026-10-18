import math
NAN=float('nan')
def isnan(v): return v!=v
CLOSE=[10.,11,12.5,11.5,13,14,13.5,15,16,15.5,17,18,17.5,19,20,19.5,21,22,21.5,23,24,23.5,25,26,25.5,27,28,27.5,29,30,29.5,31,32,31.5,33,34,33.5,35,36,35.5]
HIGH=[c+1 for c in CLOSE]; LOW=[c-1 for c in CLOSE]
N=len(CLOSE)
def sma(src,l):
    out=[NAN]*len(src)
    for i in range(l-1,len(src)):
        w=src[i-l+1:i+1]
        out[i]=sum(w)/l if all(not isnan(v) for v in w) else NAN
    return out
def ema_seed_sma(src,l):
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
def ema_firstbar(src,l):
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
    parts=[]
    for v in vals:
        parts.append("f64::NAN" if isnan(v) else f"{v!r}")
    print(f"    const {name}: [f64; {len(vals)}] = [")
    line="        "
    for p in parts:
        if len(line)+len(p)+2>98: print(line.rstrip()); line="        "
        line+=p+", "
    print(line.rstrip()+"];\n")
# RSI 14
g=[0.0]*N; l_=[0.0]*N
for i in range(1,N):
    d=CLOSE[i]-CLOSE[i-1]; g[i]=max(d,0); l_[i]=max(-d,0)
g[0]=NAN; l_[0]=NAN
rg=rma(g[1:],14); rl=rma(l_[1:],14)
rsi=[NAN]+[NAN]*len(rg)
for i,(a,b) in enumerate(zip(rg,rl)):
    if isnan(a) or isnan(b): continue
    rsi[i+1]=100.0 if b==0 else 100-100/(1+a/b)
arr("RSI",rsi)
# MACD
e12=ema_seed_sma(CLOSE,12); e26=ema_seed_sma(CLOSE,26)
macd=[(a-b) if (not isnan(a) and not isnan(b)) else NAN for a,b in zip(e12,e26)]
sig=[NAN]*N
vals=[v for v in macd if not isnan(v)]
if len(vals)>=9:
    sub=ema_seed_sma(vals,9)
    j=0
    for i,v in enumerate(macd):
        if not isnan(v): sig[i]=sub[j]; j+=1
arr("MACD",macd); arr("MACD_SIG",sig)
# STOCH
def highest(src,l):
    out=[NAN]*len(src)
    for i in range(l-1,len(src)):
        w=src[i-l+1:i+1]
        out[i]=max(w) if all(not isnan(v) for v in w) else NAN
    return out
def lowest(src,l):
    out=[NAN]*len(src)
    for i in range(l-1,len(src)):
        w=src[i-l+1:i+1]
        out[i]=min(w) if all(not isnan(v) for v in w) else NAN
    return out
hh=highest(HIGH,14); ll=lowest(LOW,14)
rawk=[NAN]*N
for i in range(N):
    if isnan(hh[i]) or isnan(ll[i]) or hh[i]==ll[i]: continue
    rawk[i]=(CLOSE[i]-ll[i])/(hh[i]-ll[i])*100
k=sma(rawk,1)  # smooth=1 -> identity where defined
d=sma(k,3)
arr("STOCH_K",k); arr("STOCH_D",d)
# STOCH RSI
hh_r=highest(rsi,14); ll_r=lowest(rsi,14)
raw=[NAN]*N
for i in range(N):
    if isnan(hh_r[i]) or isnan(ll_r[i]) or isnan(rsi[i]) or hh_r[i]==ll_r[i]: continue
    raw[i]=(rsi[i]-ll_r[i])/(hh_r[i]-ll_r[i])*100
k2=sma(raw,1); d2=sma(k2,3)
arr("STOCHRSI_K",k2); arr("STOCHRSI_D",d2)
# CMO 20
chg=[NAN]+[CLOSE[i]-CLOSE[i-1] for i in range(1,N)]
cmo=[NAN]*N
for i in range(20,N+1-1):
    pass
for i in range(1,N):
    if i<20: continue
    w=chg[i-19:i+1]
    if any(isnan(v) for v in w): continue
    up=sum(v for v in w if v>0); dn=sum(-v for v in w if v<0)
    tot=up+dn
    cmo[i]=100*(up-dn)/tot if tot!=0 else NAN
arr("CMO",cmo)
# WILLR
w=[NAN]*N
for i in range(N):
    if isnan(hh[i]) or isnan(ll[i]) or hh[i]==ll[i]: continue
    w[i]=-100*(hh[i]-CLOSE[i])/(hh[i]-ll[i])
arr("WILLR",w)
# CCI hlc3 20
tp=[(HIGH[i]+LOW[i]+CLOSE[i])/3 for i in range(N)]
tp_sma=sma(tp,20)
cci=[NAN]*N
for i in range(N):
    if isnan(tp_sma[i]): continue
    wnd=tp[i-19:i+1]
    if any(isnan(v) for v in wnd): continue
    mdev=sum(abs(v-tp_sma[i]) for v in wnd)/20
    cci[i]=(tp[i]-tp_sma[i])/(0.015*mdev) if mdev!=0 else NAN
arr("CCI",cci)
# ADX 14
dm_p=[NAN]*N; dm_m=[NAN]*N; tr=[NAN]*N
for i in range(1,N):
    up=HIGH[i]-HIGH[i-1]; dn=LOW[i-1]-LOW[i]
    dm_p[i]=up if (up>dn and up>0) else 0.0
    dm_m[i]=dn if (dn>up and dn>0) else 0.0
    tr[i]=max(HIGH[i]-LOW[i],abs(HIGH[i]-CLOSE[i-1]),abs(LOW[i]-CLOSE[i-1]))
tr[0]=HIGH[0]-LOW[0]
atr=rma(tr,14); pdi=[NAN]*N; mdi=[NAN]*N; dx=[NAN]*N; adx=[NAN]*N
pr=rma(dm_p,14); mr=rma(dm_m,14)
for i in range(N):
    if isnan(atr[i]) or atr[i]==0: continue
    pdi[i]=100*pr[i]/atr[i]; mdi[i]=100*mr[i]/atr[i]
    s=pdi[i]+mdi[i]
    dx[i]=100*abs(pdi[i]-mdi[i])/s if s!=0 else NAN
adx_arr=[NAN]*N
dxv=[(i,v) for i,v in enumerate(dx) if not isnan(v)]
if dxv:
    sub=rma([v for _,v in dxv],14)
    for (i,_),v in zip(dxv,sub): adx_arr[i]=v
arr("ADX",adx_arr); arr("DI_P",pdi); arr("DI_M",mdi)
# AO 5/34 hl2
hl2=[(HIGH[i]+LOW[i])/2 for i in range(N)]
ao=[(a-b) if (not isnan(a) and not isnan(b)) else NAN for a,b in zip(sma(hl2,5),sma(hl2,34))]
arr("AO",ao)
# ULTIMATE OSC 7/14/28
bp=[NAN]*N; trn=[NAN]*N
for i in range(1,N):
    bp[i]=CLOSE[i]-min(LOW[i],CLOSE[i-1])
    trn[i]=max(HIGH[i],CLOSE[i-1])-min(LOW[i],CLOSE[i-1])
def uo_rs(l):
    out=[NAN]*N
    for i in range(l-1,N):
        bsum=sum(bp[i-l+1:i+1]); tsum=sum(trn[i-l+1:i+1])
        out[i]=bsum/tsum if tsum!=0 else NAN
    return out
r7,r14,r28=uo_rs(7),uo_rs(14),uo_rs(28)
uo=[NAN]*N
for i in range(N):
    if not any(isnan(v) for v in (r7[i],r14[i],r28[i])):
        uo[i]=100*(4*r7[i]+2*r14[i]+r28[i])/7
arr("UO",uo)
# KST 10/15/20/30 (roc sma1), signal16
def roc_pct(src,l):
    out=[NAN]*len(src)
    for i in range(l,len(src)): out[i]=(src[i]-src[i-l])/abs(src[i-l])*100
    return out
parts=[sma(roc_pct(CLOSE,n),m) for n,m in [(10,1),(15,1),(20,1),(30,1)]]
kst=[NAN]*N
for i in range(N):
    vs=[p[i] for p in parts]
    if all(not isnan(v) for v in vs): kst[i]=sum(vs)
kst_sig=sma(kst,16)
arr("KST",kst); arr("KST_SIG",kst_sig)
# TSI 13/25 sig13
mom=[NAN]+[CLOSE[i]-CLOSE[i-1] for i in range(1,N)]
t1=ema_firstbar(mom,25); t2=ema_firstbar(t1,13)
a1=ema_firstbar([abs(v) if not isnan(v) else NAN for v in mom],25); a2=ema_firstbar(a1,13)
tsi=[NAN]*N
for i in range(N):
    if not any(isnan(v) for v in (t2[i],a2[i])) and a2[i]!=0:
        tsi[i]=100*t2[i]/a2[i]
tsi_sig=ema_firstbar(tsi,13)
arr("TSI",tsi); arr("TSI_SIG",tsi_sig)
# RVI len10: num=4*c-h-l-2*c[1] variant: standard: ema( (c-l)-(h-c) )*? use classical:
num_src=[NAN]+[4*CLOSE[i]-HIGH[i]-LOW[i]-2*CLOSE[i-1] for i in range(1,N)]
den_src=[HIGH[i]-LOW[i] for i in range(N)]
rvi=[NAN]*N
e_n=ema_seed_sma(num_src,10); e_d=ema_seed_sma(den_src,10)
for i in range(N):
    if not any(isnan(v) for v in (e_n[i],e_d[i])) and e_d[i]!=0:
        rvi[i]=e_n[i]/e_d[i]
arr("RVI",rvi)
# TRIX 9 sig9
te1=ema_seed_sma(CLOSE,9); te2=ema_seed_sma(te1,9); te3=ema_seed_sma(te2,9)
trix=[NAN]*N
for i in range(1,N):
    if not isnan(te3[i]) and not isnan(te3[i-1]) and te3[i-1]!=0:
        trix[i]=(te3[i]-te3[i-1])/abs(te3[i-1])*10000
trix_sig=[NAN]*N
tv=[(i,v) for i,v in enumerate(trix) if not isnan(v)]
if tv:
    sub=ema_seed_sma([v for _,v in tv],9)
    for (i,_),v in zip(tv,sub): trix_sig[i]=v
arr("TRIX",trix); arr("TRIX_SIG",trix_sig)
# DPO 20: c - sma(c,20) shifted center=len/2+1=11: dpo[i]=c[i]-sma[i-11+1?] centered: subtract sma evaluated at i - offset +1 where offset=len/2+1=11
s20=sma(CLOSE,20)
dpo=[NAN]*N
for i in range(N):
    j=i-11+1
    if j>=0 and not isnan(s20[j]): dpo[i]=CLOSE[i]-s20[j]
arr("DPO",dpo)
print("N =",N)
