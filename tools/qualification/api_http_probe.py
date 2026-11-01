#!/usr/bin/env python3
import argparse,os,secrets,signal,socket,subprocess,time,json,tempfile,urllib.request,urllib.error
from pathlib import Path
parser=argparse.ArgumentParser(description='Probe the actual FusionPanel API process over HTTP; no mock server is used.')
parser.add_argument('--binary', required=True, type=Path)
parser.add_argument('--output', type=Path)
parser.add_argument('--expected-version', default='0.1.0-dev.1')
args=parser.parse_args()
binary=args.binary.resolve()
assert binary.is_file(),'Actual API binary is required'
with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
token=secrets.token_urlsafe(32)
env=dict(os.environ,FUSIONPANEL_LISTEN=f'127.0.0.1:{port}',FUSIONPANEL_ADMIN_TOKEN=token)
results=[]
with tempfile.TemporaryFile() as log:
 proc=subprocess.Popen([str(binary)],env=env,stdout=log,stderr=log)
 def request(path,auth=None):
  headers={} if auth is None else {'Authorization':f'Bearer {auth}'}
  req=urllib.request.Request(f'http://127.0.0.1:{port}{path}',headers=headers)
  try:
   with urllib.request.urlopen(req,timeout=2) as res:return res.status,json.load(res),dict(res.headers)
  except urllib.error.HTTPError as res:return res.code,json.load(res),dict(res.headers)
 try:
  end=time.monotonic()+10
  while True:
   if proc.poll() is not None:raise AssertionError('Actual API exited before readiness')
   try:request('/v1/health');break
   except (OSError,urllib.error.URLError):
    if time.monotonic()>end:raise AssertionError('Readiness deadline exceeded')
    time.sleep(.05)
  for name,path,auth,expected in [
   ('public-health','/v1/health',None,200),
   ('missing-auth','/v1/capabilities',None,401),
   ('invalid-auth','/v1/capabilities','invalid-test-token',401),
   ('valid-auth','/v1/capabilities',token,200),
   ('unknown-route','/v1/missing',None,404)]:
   status,body,headers=request(path,auth)
   assert status==expected,(name,status,expected)
   assert token not in json.dumps(body),name+' token leaked'
   assert headers.get('access-control-allow-origin')!='*','wildcard CORS'
   if name=='public-health':assert body=={'status':'ok','service':'fusionpanel-api','version':args.expected_version}
   if name=='valid-auth':assert set(body['capabilities'])=={'control-plane.health.read','control-plane.capabilities.read'}
   if status==401:assert body=={'error':{'code':'unauthorized','message':'Authentication required'}}
   if status==404:assert body['error']['code']=='not_found'
   results.append({'case':name,'result':'PASS','http_status':status})
  proc.send_signal(signal.SIGINT);rc=proc.wait(timeout=5)
  assert rc==0,('graceful-shutdown',rc)
  log.seek(0);logs=log.read().decode(errors='replace');assert token not in logs,'Token leaked in process logs'
  results.append({'case':'graceful-shutdown-and-log-redaction','result':'PASS'})
 finally:
  if proc.poll() is None:proc.kill();proc.wait(timeout=5)
summary={'test_type':'actual-process HTTP integration, not full product E2E','binary':'fusionpanel-api','cases':results,'passed':len(results),'failed':0}
if args.output:
 args.output.parent.mkdir(parents=True,exist_ok=True)
 args.output.write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary))
