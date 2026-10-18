#!/usr/bin/env python3
"""Fetch TradingView built-in study metaInfo (params, defaults, colors, plots) from pine-facade.
Dev-time reference only; each success saved to research/studies/meta/<id>.json"""
import json,os,sys,time,urllib.request,urllib.parse
UA="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
OUT=os.path.join(os.path.dirname(__file__),"studies","meta")
CANDS=[
("52 Week High/Low",["STD;52_Week_High_Low","STD;Study_ADR_52"]),
("Accelerator Oscillator",["STD;Awesome_Oscillator"]),
("Accumulation/Distribution",["STD;Accumulation_Distribution"]),
("Accumulative Swing Index",["STD;ASI","STD;Accumulative_Swing_Index"]),
("Advance/Decline",["STD;Advance_Decline_Ratio_Bars","STD;Advance_DecLine"]),
("Arnaud Legoux Moving Average",["STD;ALMA","STD;Arnaud_Legoux_Moving_Average"]),
("Aroon",["STD;Aroon"]),
("Average Directional Index",["STD;ADX","STD;Average_Directional_Index","STD;ADXR"]),
("Average Price",["STD;Average_Price"]),
("Average True Range",["STD;Average_True_Range","STD;ATR"]),
("Awesome Oscillator",["STD;Awesome_Oscillator"]),
("Balance of Power",["STD;Balance_of_Power"]),
("Bollinger Bands",["STD;Bollinger_Bands"]),
("Bollinger Bands %B",["STD;Bollinger_Bands_B"]),
("Bollinger Bands Width",["STD;Bollinger_Bands_Width"]),
("Chaikin Money Flow",["STD;Chaikin_Money_Flow"]),
("Chaikin Oscillator",["STD;Chaikin_Oscillator"]),
("Chaikin Volatility",["STD;Chaikin_Volatility"]),
("Chande Kroll Stop",["STD;Chande_Kroll_Stop"]),
("Chande Momentum Oscillator",["STD;Chande_Momentum_Oscillator"]),
("Chop Zone",["STD;Chop_Zone"]),
("Choppiness Index",["STD;Choppiness_Index"]),
("Commodity Channel Index",["STD;CCI"]),
("Connors RSI",["STD;Connors_RSI","STD;CRSI"]),
("Coppock Curve",["STD;Coppock_Curve"]),
("Correlation Coefficient",["STD;Correlation_Coeff"]),
("Correlation - Log",["STD;Correlation_Log"]),
("Detrended Price Oscillator",["STD;DPO"]),
("Directional Movement",["STD;DMI","STD;DM"]),
("Donchian Channels",["STD;Donchian_Channels"]),
("Double EMA",["STD;DEMA","STD;Double_EMA"]),
("Ease of Movement",["STD;EOM","STD;Ease_of_Movement"]),
("Elder's Force Index",["STD;EFI"]),
("EMA Cross",["STD;EMA_Cross"]),
("Envelopes",["STD;ENV","STD;Envelopes"]),
("Fisher Transform",["STD;Fisher_Transform"]),
("Guppy Multiple Moving Average",["STD;GMMA","STD;Guppy_MMA"]),
("Historical Volatility",["STD;Historical_Volatility","STD;HV"]),
("Hull Moving Average",["STD;Hull_MA","STD;HMA","STD;hullMA"]),
("Ichimoku Cloud",["STD;Ichimoku_Cloud","STD;IchimokuCloud"]),
("Keltner Channels",["STD;Keltner_Channels"]),
("Klinger Oscillator",["STD;Klinger_Oscillator"]),
("Know Sure Thing",["STD;Know_Sure_Thing","STD;KST"]),
("Least Squares Moving Average",["STD;LSMA","STD;Least_Squares_Moving_Average"]),
("Linear Regression Curve",["STD;Linear_Regression"]),
("Linear Regression Slope",["STD;Linear_Regression_Slope"]),
("MA Cross",["STD;MA_Cross","STD;Moving_Average_Cross"]),
("MA with EMA Cross",["STD;MA_with_EMA_Cross"]),
("Mass Index",["STD;Mass_Index"]),
("McGinley Dynamic",["STD;McGinley_Dynamic"]),
("Median Price",["STD;Median_Price"]),
("Momentum",["STD;Momentum"]),
("Money Flow Index",["STD;Money_Flow"]),
("Moving Average",["STD;SMA"]),
("Moving Average Channel",["STD;MA_Channel","STD;Moving_Average_Channel"]),
("Moving Average Exponential",["STD;MAE","STD;EMA"]),
("Moving Average Weighted",["STD;MAW","STD;WMA"]),
("Moving Average Double",["STD;MAD","STD;DMA"]),
("Moving Average Triple",["STD;MAT","STD;TEMA"]),
("Moving Average Adaptive",["STD;AMA","STD;KAMA"]),
("Moving Average Hamming",["STD;HAM","STD;Moving_Average_Hamming"]),
("Moving Average Multiple",["STD;MM","STD;Moving_Average_Multiple"]),
("Majority Rule",["STD;Majority_Rule"]),
("Net Volume",["STD;Net_Volume"]),
("On Balance Volume",["STD;On_Balance_Volume"]),
("Parabolic SAR",["STD;PSAR"]),
("Pivot Points Standard",["STD;PivotPointsStandard","STD;Pivot_Points_Standard"]),
("Price Channel",["STD;Price_Channel","STD;MTPC"]),
("Price Oscillator",["STD;Price_Oscillator"]),
("Price Volume Trend",["STD;Price_Volume_Trend"]),
("Rank Correlation Index",["STD;RCI","STD;Rank_Correlation_Index"]),
("Rate Of Change",["STD;ROC"]),
("Ratio",["STD;Ratio","STD;RS"]),
("Relative Strength Index",["STD;RSI"]),
("Relative Vigor Index",["STD;Relative_Vigor_Index"]),
("Relative Volatility Index",["STD;Relative_Volatility_Index"]),
("Standard Error",["STD;Standard_Error"]),
("Standard Error Bands",["STD;SEBands","STD;Standard_Error_Bands"]),
("SMI Ergodic Indicator/Oscillator",["STD;SMI_Ergodic_Indicator_Oscillator"]),
("Smoothed Moving Average",["STD;SMMA"]),
("Standard Deviation",["STD;Standard_Deviation"]),
("Stochastic",["STD;Stochastic"]),
("Stochastic RSI",["STD;Stochastic_RSI"]),
("Session Volume Profile",["STD;Session_Volume_Profile"]),
("SuperTrend",["STD;SuperTrend"]),
("Spread",["STD;Spread"]),
("TRIX",["STD;TRIX"]),
("Triple EMA",["STD;TEMA"]),
("True Strength Index",["STD;True_Strength_Index"]),
("Trend Strength Index",["STD;Trend_Strength_Index"]),
("Typical Price",["STD;Typical_Price"]),
("Ultimate Oscillator",["STD;Ultimate_Oscillator"]),
("Volatility Close-to-Close",["STD;Volatility_Close_to_Close"]),
("Volatility Zero Trend Close-to-Close",["STD;VZT_close_to_close"]),
("Volatility O-H-L-C",["STD;Volatility_O_H_L_C"]),
("Volatility Index",["STD;Volatility_Index"]),
("VWAP",["STD;VWAP"]),
("VWMA",["STD;VWMA"]),
("Volume Oscillator",["STD;Volume_Oscillator"]),
("Volume Profile Fixed Range",["STD;Volume_Profile_Fixed_Range"]),
("Volume Profile Visible Range",["STD;Volume_Profile_Visible_Range"]),
("Vortex Indicator",["STD;Vortex_Indicator"]),
("Williams %R",["STD;Williams_R","STD;Willams_R"]),
("Williams Alligator",["STD;Williams_Alligator"]),
("Williams Fractal",["STD;Williams_Fractal","STD;Whilliams_Fractals"]),
("Zig Zag",["STD;Zig_Zag"]),
("Volatility Stop",["STD;Volatility_Stop"]),
("Chandelier Exit",["STD;Chandelier_Exit"]),
("Darvas Box",["STD;Darvas_Box"]),
("Elder's Ray Index",["STD;Elder_Ray","STD;Elder_s_Ray_Index"]),
("Random Walk Index",["STD;Random_Walk_Index"]),
("Chaikin Money Flow 2",[]),
]
def fetch(sid):
    u="https://pine-facade.tradingview.com/pine-facade/translate/"+urllib.parse.quote(sid,safe='')+"/last"
    req=urllib.request.Request(u,headers={"User-Agent":UA,"Referer":"https://www.tradingview.com/","Accept":"application/json"})
    for attempt in range(2):
        try:
            with urllib.request.urlopen(req,timeout=10) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            if e.code in (403,429):
                time.sleep(20); continue
            return None
        except Exception:
            time.sleep(3); continue
    return None
ok=0;fail=[]
for name,ids in CANDS:
    for sid in ids:
        fn=os.path.join(OUT,sid.replace(';','_')+".json")
        if os.path.exists(fn): ok+=1; break
        d=fetch(sid)
        if d and d.get('success') and 'metaInfo' in d.get('result',{}):
            meta=d['result']['metaInfo']
            json.dump({"name":name,"scriptIdPart":sid,"metaInfo":meta},open(fn,'w'))
            ok+=1; break
        time.sleep(1.0)
    else:
        fail.append(name)
    time.sleep(0.4)
print(json.dumps({"ok":ok,"failed":fail}))
