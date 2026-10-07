"""Windows resource sampler for the opt-in Rust soak test. Requires psutil.
Run from the repository with the local Rust toolchain on PATH.
"""
import argparse, json, os, pathlib, subprocess, time
import psutil

p=argparse.ArgumentParser()
p.add_argument('--report-dir',type=pathlib.Path,default=pathlib.Path('.local/soak'))
p.add_argument('--quick',action='store_true',help='Harness smoke only: 20-minute media, 45-second outage')
p.add_argument('--max-rss-mib',type=int,default=256,help='Fail if engine plus synthetic server RSS exceeds this budget')
a=p.parse_args();a.report_dir.mkdir(parents=True,exist_ok=True)
env=os.environ.copy()
if a.quick:env.update(SOAK_MEDIA_SECONDS='1200',SOAK_OUTAGE_SECONDS='45')
result=subprocess.run(['cargo','test','-p','video-engine','--test','soak','--no-run','--message-format=json'],capture_output=True,text=True,check=True,env=env)
binary=None
for line in result.stdout.splitlines():
 try:r=json.loads(line)
 except json.JSONDecodeError:continue
 if r.get('reason')=='compiler-artifact' and r.get('target',{}).get('name')=='soak':binary=r.get('executable')
if not binary:raise RuntimeError('No soak test executable')
logpath=a.report_dir/'soak.log'
samples=[];start=time.monotonic();offset=0
with logpath.open('w',encoding='utf-8') as log:
 process=subprocess.Popen([binary,'--ignored','--nocapture','--test-threads=1'],stdout=log,stderr=subprocess.STDOUT,env=env)
 root=psutil.Process(process.pid)
 while process.poll() is None:
  try:
   engine=root.memory_info();child_rss=sum(c.memory_info().rss for c in root.children(recursive=True) if c.is_running())
   sample={'seconds':round(time.monotonic()-start,2),'engineServerRssBytes':engine.rss,'engineServerPrivateBytes':getattr(engine,'private',None),'mediaChildRssBytes':child_rss}
   samples.append(sample)
  except (psutil.NoSuchProcess,psutil.AccessDenied):pass
  with logpath.open(encoding='utf-8',errors='replace') as read:
   read.seek(offset);new=read.read();offset=read.tell()
  if new:print(new,end='',flush=True)
  time.sleep(1)
code=process.returncode
peak=max((s['engineServerRssBytes'] for s in samples),default=0)
if peak>a.max_rss_mib*1024*1024:
 print(f'RSS budget exceeded: {peak} bytes',flush=True)
 code=code or 1
phases=[]
for line in logpath.read_text(encoding='utf-8',errors='replace').splitlines():
 if 'SOAK ' in line:phases.append(json.loads(line.split('SOAK ',1)[1]))
report={'exitCode':code,'maxRssMiB':a.max_rss_mib,'quick':a.quick,'wallSeconds':round(time.monotonic()-start,2),'scope':'Engine and in-process synthetic HTTP server; excludes Tauri/WebView UI. Loopback payload bytes, not internet link utilization.','peakEngineServerRssBytes':max((s['engineServerRssBytes'] for s in samples),default=0),'peakMediaChildRssBytes':max((s['mediaChildRssBytes'] for s in samples),default=0),'phases':phases,'samples':samples}
(a.report_dir/'soak.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k!='samples'},indent=2),flush=True)
raise SystemExit(code)
