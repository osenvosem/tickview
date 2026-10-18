import math
NAN=float('nan')
def isnan(v): return v!=v
CLOSE=[10.,11,12.5,11.5,13,14,13.5,15,16,15.5,17,18,17.5,19,20,19.5,21,22,21.5,23,24,23.5,25,26,25.5,27,28,27.5,29,30,29.5,31,32,31.5,33,34,33.5,35,36,35.5]
HIGH=[c+1 for c in CLOSE]; LOW=[c-1 for c in CLOSE]
VOL=[100.,150,120,180,140,160,130,170,150,190,110,140,160,120,180,150,130,170,140,160,
     100,150,120,180,140,160,130,170,150,190,110,140,160,120,180,150,130,170,140,160]
N=len(CLOSE)
DAY=[i//10 for i in range(N)]
OPEN=[CLOSE[0]]+[CLOSE[i-1] for i in range(1,N)]
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
def shift_right(src,k):
    out=[NAN]*len(src)
    for i in range(k,len(src)): out[i]=src[i-k]
    return out
def arr(name,vals):
    parts=["f64::NAN" if isnan(v) else f"{float(v)!r}" for v in vals]
    print(f"    const {name}: [f64; {len(vals)}] = [")
    line="        "
    for p in parts:
        if len(line)+len(p)+2>98: print(line.rstrip()); line="        "
        line+=p+", "
    print(line.rstrip()+"];\n")
def arr_i(name,vals):
    parts=["i32::MIN" if v is None else str(v) for v in vals]
    print(f"    const {name}: [i32; {len(vals)}] = [")
    line="        "
    for p in parts:
        if len(line)+len(p)+2>98: print(line.rstrip()); line="        "
        line+=p+", "
    print(line.rstrip()+"];\n")

tr=[HIGH[0]-LOW[0]]
for i in range(1,N):
    tr.append(max(HIGH[i]-LOW[i], abs(HIGH[i]-CLOSE[i-1]), abs(LOW[i]-CLOSE[i-1])))

# PSAR
def psar(h,l,af0=0.02,inc=0.02,maxaf=0.2):
    n=len(h); out=[NAN]*n
    if n<2: return out
    bull=True; af=af0; ep=h[0]; sar=l[0]
    for i in range(1,n):
        s=sar+af*(ep-sar)
        if bull:
            s=min(s,l[i-1])
            if i>=2: s=min(s,l[i-2])
            if l[i]<s:
                bull=False; s=ep; ep=l[i]; af=af0
            else:
                if h[i]>ep: ep=h[i]; af=min(af+inc,maxaf)
        else:
            s=max(s,h[i-1])
            if i>=2: s=max(s,h[i-2])
            if h[i]>s:
                bull=True; s=ep; ep=h[i]; af=af0
            else:
                if l[i]<ep: ep=l[i]; af=min(af+inc,maxaf)
        out[i]=s; sar=s
    return out
arr("PSAR",psar(HIGH,LOW))
# Supertrend
hl2=[(HIGH[i]+LOW[i])/2 for i in range(N)]
atr=rma(tr,10)
ub=[hl2[i]+3*atr[i] if not isnan(atr[i]) else NAN for i in range(N)]
lb=[hl2[i]-3*atr[i] if not isnan(atr[i]) else NAN for i in range(N)]
fub=[NAN]*N; flb=[NAN]*N; trn=[NAN]*N; st=[NAN]*N
prev_t=1
for i in range(N):
    if isnan(ub[i]): continue
    fub[i]=ub[i] if (i==0 or isnan(fub[i-1]) or ub[i]<fub[i-1] or CLOSE[i-1]>fub[i-1]) else fub[i-1]
    flb[i]=lb[i] if (i==0 or isnan(flb[i-1]) or lb[i]>flb[i-1] or CLOSE[i-1]<flb[i-1]) else flb[i-1]
    if prev_t==1 and CLOSE[i]<=flb[i]: trn[i]=-1
    elif prev_t==-1 and CLOSE[i]>=fub[i]: trn[i]=1
    else: trn[i]=prev_t
    st[i]=flb[i] if trn[i]==1 else fub[i]
    prev_t=trn[i]
arr("SUPERTREND",st); arr("ST_TREND",trn)
# Alligator
jaw=rma(hl2,13); teeth=rma(hl2,8); lips=rma(hl2,5)
arr("ALLIG_JAW",shift_right(jaw,8)); arr("ALLIG_TEETH",shift_right(teeth,3)); arr("ALLIG_LIPS",shift_right(lips,1))
# Vortex
vip=[NAN]*N; vim=[NAN]*N
for i in range(14,N):
    num_p=sum(abs(HIGH[j]-LOW[j-1]) for j in range(i-13,i+1))
    num_m=sum(abs(LOW[j]-HIGH[j-1]) for j in range(i-13,i+1))
    den=sum(tr[j] for j in range(i-13,i+1))
    if den!=0: vip[i]=num_p/den; vim[i]=num_m/den
arr("VORTEX_P",vip); arr("VORTEX_M",vim)
# Bollinger (20,2) population stdev
basis=sma(CLOSE,20)
ub_=[NAN]*N; lb_=[NAN]*N; pctb=[NAN]*N; bw=[NAN]*N
for i in range(19,N):
    w=CLOSE[i-19:i+1]
    m=sum(w)/20
    sd=math.sqrt(sum((x-m)**2 for x in w)/20)
    ub_[i]=m+2*sd; lb_[i]=m-2*sd
    pctb[i]=(CLOSE[i]-lb_[i])/(ub_[i]-lb_[i]) if ub_[i]!=lb_[i] else NAN
    bw[i]=(ub_[i]-lb_[i])/m if m!=0 else NAN
arr("BB_UP",ub_); arr("BB_LOW",lb_); arr("BB_PCTB",pctb); arr("BB_BW",bw); arr("BB_BASIS",basis)
# Keltner (20,2) EMA basis + RMA range
kb=ema_sma(CLOSE,20); ra=rma(tr,20)
arr("KC_MID",kb)
arr("KC_UP",[kb[i]+2*ra[i] if not isnan(kb[i]) and not isnan(ra[i]) else NAN for i in range(N)])
arr("KC_LOW",[kb[i]-2*ra[i] if not isnan(kb[i]) and not isnan(ra[i]) else NAN for i in range(N)])
# Donchian 20
dcu=[NAN]*N; dcl=[NAN]*N
for i in range(19,N): dcu[i]=max(HIGH[i-19:i+1]); dcl[i]=min(LOW[i-19:i+1])
arr("DON_UP",dcu); arr("DON_LOW",dcl)
# Envelope SMA(20) +/- 5%
env=[NAN]*N
for i in range(19,N):
    m=sum(CLOSE[i-19:i+1])/20
    env[i]=m
arr("ENV_BASIS",env)
arr("ENV_UP",[env[i]*1.05 if not isnan(env[i]) else NAN for i in range(N)])
arr("ENV_LOW",[env[i]*0.95 if not isnan(env[i]) else NAN for i in range(N)])
# VWAP with session-day index + std dev bands
vw=[NAN]*N; vsd=[NAN]*N
pvv=0.0; vv=0.0; prev_d=None
pv2=0.0
for i in range(N):
    d=DAY[i]
    if d!=prev_d:
        pvv=0.0; vv=0.0; prev_d=d
    tpc=(HIGH[i]+LOW[i]+CLOSE[i])/3
    pvv+=tpc*VOL[i]; vv+=VOL[i]
    vw[i]=pvv/vv if vv!=0 else NAN
# std dev over the WHOLE series (single anchor chain) — per-day anchored:
sd_all=[NAN]*N
pvv=0.0; vv=0.0; pv2=0.0; prev_d=None
for i in range(N):
    d=DAY[i]
    if d!=prev_d: pvv=0.0; vv=0.0; pv2=0.0; prev_d=d
    tpc=(HIGH[i]+LOW[i]+CLOSE[i])/3
    pvv+=tpc*VOL[i]; vv+=VOL[i]; pv2+=tpc*tpc*VOL[i]
    if vv!=0:
        m=pvv/vv
        var=pv2/vv-m*m
        sd_all[i]=math.sqrt(max(var,0.0))
arr("VWAP",vw); arr("VWAP_SD",sd_all)
# Linear regression channel len=20, mult=1
lrc=[NAN]*N; lru=[NAN]*N; lrl=[NAN]*N
for i in range(19,N):
    w=CLOSE[i-19:i+1]; xs=list(range(20))
    mx=sum(xs)/20; my=sum(w)/20
    sxx=sum((x-mx)**2 for x in xs); sxy=sum((xs[k]-mx)*(w[k]-my) for k in range(20))
    slope=sxy/sxx if sxx!=0 else 0.0
    icpt=my-slope*mx
    fit=[icpt+slope*x for x in xs]
    resid=[w[k]-fit[k] for k in range(20)]
    sd=math.sqrt(sum(r*r for r in resid)/20)
    lrc[i]=fit[-1]; lru[i]=fit[-1]+sd; lrl[i]=fit[-1]-sd
arr("LRC",lrc); arr("LRU",lru); arr("LRL",lrl)
# Mass Index
b=ema_fb([HIGH[i]-LOW[i] for i in range(N)],9)
b1=ema_fb(b,9)
ratio=[b[i]/b1[i] if not isnan(b[i]) and not isnan(b1[i]) and b1[i]!=0 else NAN for i in range(N)]
mi=[NAN]*N
for i in range(24,N):
    w=ratio[i-24:i+1]
    if all(not isnan(x) for x in w): mi[i]=sum(w)
arr("MASS",mi); arr("MASS_SIGNAL",sma(mi,9))
# HV close-to-close: stdev(ln returns, len)*sqrt(252)*100
lr=[NAN]+[math.log(CLOSE[i]/CLOSE[i-1]) for i in range(1,N)]
hv=[NAN]*N
for i in range(20,N):
    w=lr[i-19:i+1]
    if all(not isnan(x) for x in w):
        m=sum(w)/20
        hv[i]=math.sqrt(sum((x-m)**2 for x in w)/20)*math.sqrt(252)*100
arr("HV",hv)
# Parkinson 55
park=[NAN]*N
for i in range(54,N):
    s=sum(math.log(HIGH[j]/LOW[j])**2 for j in range(i-54,i+1))
    park[i]=math.sqrt(s/(55*4*math.log(2)))*math.sqrt(252)*100
arr("PARKINSON",park)
# Chaikin Volatility: EMA(smooth,first-bar) of ROC(EMA(H-L,len),len)*100
sprd=[HIGH[i]-LOW[i] for i in range(N)]
ma=ema_sma(sprd,20)
cv_raw=[NAN]*N
for i in range(N):
    j=i-20
    if j>=0 and not isnan(ma[i]) and not isnan(ma[j]) and ma[j]!=0:
        cv_raw[i]=(ma[i]-ma[j])/ma[j]*100
arr("CHAIKIN_VOL",ema_fb(cv_raw,3))
# Garman-Klass 20
gk=[NAN]*N
for i in range(19,N):
    s=0.0
    for j in range(i-19,i+1):
        hl=math.log(HIGH[j]/LOW[j]); co=math.log(CLOSE[j]/OPEN[j])
        s+=0.5*hl*hl-math.log(2)*co*co
    gk[i]=math.sqrt(s/20)*math.sqrt(252)*100
arr("GARMAN",gk)
# Rogers-Satchell 20
rs=[NAN]*N
for i in range(19,N):
    s=0.0
    for j in range(i-19,i+1):
        hc=math.log(HIGH[j]/CLOSE[j]); lc=math.log(LOW[j]/CLOSE[j]); ho=math.log(HIGH[j]/OPEN[j]); lo=math.log(LOW[j]/OPEN[j])
        s+=hc*lc+ho*lo
    rs[i]=math.sqrt(max(s/20,0.0))*math.sqrt(252)*100
arr("ROGERS",rs)
# Pivots daily (anchor=DAY): day H/L, prev H/L/C + line families
days=sorted(set(DAY))
day_hl={}; day_close={}
for i in range(N):
    d=DAY[i]
    if d not in day_hl: day_hl[d]=[HIGH[i],LOW[i]]; day_close[d]=CLOSE[i]
    else:
        day_hl[d][0]=max(day_hl[d][0],HIGH[i]); day_hl[d][1]=min(day_hl[d][1],LOW[i]); day_close[d]=CLOSE[i]
def piv(name):
    out=[NAN]*N
    for i in range(N):
        v=name(i)
        if v is not None: out[i]=v
    arr_i(name2, out) if False else None
    return out
DH=[NAN]*N; DL=[NAN]*N; PC=[NAN]*N; PH=[NAN]*N; PL=[NAN]*N
PP=[NAN]*N
CR1=[NAN]*N; CR2=[NAN]*N; CR3=[NAN]*N; CR4=[NAN]*N; CS1=[NAN]*N; CS2=[NAN]*N; CS3=[NAN]*N; CS4=[NAN]*N
WP=[NAN]*N; WR1=[NAN]*N; WS1=[NAN]*N
FR1=[NAN]*N; FR2=[NAN]*N; FR3=[NAN]*N; FS1=[NAN]*N; FS2=[NAN]*N; FS3=[NAN]*N
for i in range(N):
    prev=DAY[i]-1
    if prev not in day_hl: continue
    hh,ll=day_hl[prev]; cc=day_close[prev]
    rng=hh-ll
    p=(hh+ll+cc)/3
    DH[i]=hh; DL[i]=ll; PC[i]=cc; PH[i]=hh; PL[i]=ll; PP[i]=p
    CR1[i]=cc+rng*(1.1/12); CR2[i]=cc+rng*(1.1/6); CR3[i]=cc+rng*(1.1/4); CR4[i]=cc+rng*(1.1/2)
    CS1[i]=cc-rng*(1.1/12); CS2[i]=cc-rng*(1.1/6); CS3[i]=cc-rng*(1.1/4); CS4[i]=cc-rng*(1.1/2)
    WP[i]=(hh+ll+2*cc)/4; WR1[i]=2*WP[i]-ll; WS1[i]=2*WP[i]-hh
    FR1[i]=p+0.382*rng; FR2[i]=p+0.618*rng; FR3[i]=p+rng
    FS1[i]=p-0.382*rng; FS2[i]=p-0.618*rng; FS3[i]=p-rng
arr("PIV_DH",DH); arr("PIV_DL",DL); arr("PIV_PC",PC); arr("PIV_PH",PH); arr("PIV_PL",PL); arr("PIV_P",PP)
arr("PIV_CR1",CR1); arr("PIV_CR2",CR2); arr("PIV_CR3",CR3); arr("PIV_CR4",CR4)
arr("PIV_CS1",CS1); arr("PIV_CS2",CS2); arr("PIV_CS3",CS3); arr("PIV_CS4",CS4)
arr("PIV_WP",WP); arr("PIV_WR1",WR1); arr("PIV_WS1",WS1)
arr("PIV_FR1",FR1); arr("PIV_FR2",FR2); arr("PIV_FR3",FR3); arr("PIV_FS1",FS1); arr("PIV_FS2",FS2); arr("PIV_FS3",FS3)
print("N =",N)
