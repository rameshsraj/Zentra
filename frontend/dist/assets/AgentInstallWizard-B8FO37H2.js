import{j as e,u as S,c as E}from"./vendor-query-BrH7-v9p.js";import{r as n,b as G}from"./vendor-react-BpE_XXTR.js";import{b as K,r as W,u as U,e as B,c as V,a as Q,f as X,h as Y,l as H,g as J}from"./fleet-BOF2BzBl.js";import{P as ee}from"./PageHeader-B-r-mE4Q.js";import{S as z,_ as $,T as te,aZ as Z,O as T,L as A,al as F,aR as O,ab as q,aX as D,R as M,o as se,q as re,bk as ne,N as oe,G as ae,D as le}from"./vendor-ui-AQemR2AI.js";import"./index-C6gmLKkE.js";import"./vendor-utils-D19diF5V.js";const ie=["Register Datacenter","Install Agent","Waiting for Connection","Targets Discovered"];function ce(t){const s=t.toUpperCase();return s==="ERROR"?"text-red-400":s==="WARN"||s==="WARNING"?"text-amber-400":s==="DEBUG"||s==="TRACE"?"text-zinc-500":"text-emerald-400"}function P({dcName:t,defaultOpen:s=!0}){const[o,l]=n.useState(s),[x,m]=n.useState(!1),[c,h]=n.useState([]),f=n.useRef(null),b=n.useRef(new Set),{data:u}=S({queryKey:["agent-logs",t],queryFn:()=>Q(t),refetchInterval:x||!o?!1:3e3,enabled:!!t});return n.useEffect(()=>{if(!(u!=null&&u.lines))return;const r=b.current,N=[];for(const p of u.lines){const d=`${p.timestamp}|${p.message}`;r.has(d)||(r.add(d),N.push(p))}N.length>0&&h(p=>{const d=[...p,...N];return d.length>500?d.slice(d.length-500):d})},[u]),n.useEffect(()=>{if(x)return;const r=f.current;r&&(r.scrollTop=r.scrollHeight)},[c,x]),e.jsxs("div",{className:"rounded-lg border border-border bg-card",children:[e.jsxs("button",{onClick:()=>l(r=>!r),className:"w-full flex items-center gap-2 px-4 py-2.5 text-sm font-medium hover:bg-muted/40 transition-colors",children:[e.jsx(se,{className:"h-4 w-4 text-blue-400"}),e.jsx("span",{className:"flex-1 text-left",children:"Agent Logs (live tail)"}),e.jsxs("span",{className:"text-xs text-muted-foreground",children:[c.length," lines"]}),o?e.jsx(q,{className:"h-4 w-4"}):e.jsx(T,{className:"h-4 w-4"})]}),o&&e.jsxs("div",{className:"border-t border-border",children:[e.jsxs("div",{className:"flex items-center justify-between px-4 py-2 text-xs border-b border-border",children:[e.jsxs("span",{className:"text-muted-foreground",children:["Polling every 3s · datacenter ",e.jsx("code",{className:"text-xs",children:t})]}),e.jsxs("div",{className:"flex items-center gap-2",children:[e.jsx("button",{onClick:()=>m(r=>!r),className:"rounded px-2 py-0.5 border border-border hover:bg-muted/40",children:x?"Resume":"Pause"}),e.jsx("button",{onClick:()=>{h([]),b.current=new Set},className:"rounded px-2 py-0.5 border border-border hover:bg-muted/40",children:"Clear"})]})]}),e.jsx("div",{ref:f,className:"h-64 overflow-y-auto bg-zinc-950 px-3 py-2 font-mono text-xs leading-relaxed",children:c.length===0?e.jsx("div",{className:"text-zinc-500 italic",children:"Waiting for agent log lines… (first heartbeat arrives within 30s of agent start)"}):c.map((r,N)=>e.jsxs("div",{className:"flex gap-2 whitespace-pre-wrap break-words",children:[e.jsx("span",{className:"text-zinc-600 shrink-0",children:new Date(r.timestamp).toLocaleTimeString()}),e.jsx("span",{className:`shrink-0 w-12 ${ce(r.level)}`,children:r.level.toUpperCase()}),e.jsx("span",{className:"text-zinc-500 shrink-0",children:r.target}),e.jsx("span",{className:"text-zinc-200",children:r.message})]},N))})]})]})}function de({onComplete:t}){const[s,o]=n.useState("datacenter"),[l,x]=n.useState(""),[m,c]=n.useState(""),[h,f]=n.useState(""),[b,u]=n.useState(!1),[r,N]=n.useState(!1),[p,d]=n.useState(""),{data:v=[]}=S({queryKey:["org-enrollment-keys"],queryFn:X,refetchInterval:3e4,enabled:s==="orgkey"}),a=v[0],{data:w=[]}=S({queryKey:["org-enrollment-registrations",a==null?void 0:a.id],queryFn:()=>Y(a.id),enabled:s==="orgkey"&&!!(a!=null&&a.id),refetchInterval:3e4}),k=E({mutationFn:()=>K({}),onSuccess:g=>d(g.org_key)}),i=E({mutationFn:g=>W(g)}),C=E({mutationFn:({id:g,value:j})=>U(g,{require_approval:j})}),y=E({mutationFn:()=>B({datacenter_name:l,region_label:m||void 0}),onSuccess:g=>f(g.api_key)}),I=()=>{navigator.clipboard.writeText(h),u(!0),setTimeout(()=>u(!1),2e3)};if(s==="orgkey"){const g=`ZENTRA_AGENT_MODE=remote \\
ZENTRA_API_ENDPOINT=${window.location.protocol}//${window.location.hostname}:5006 \\
ZENTRA_ORG_KEY=${p||`${a==null?void 0:a.display_prefix}...`} \\
./zentra-agent`;return e.jsxs("div",{className:"space-y-6",children:[e.jsxs("div",{className:"flex gap-1 rounded-lg bg-muted p-1 w-fit",children:[e.jsx("button",{onClick:()=>o("datacenter"),className:"rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground hover:text-foreground",children:"Register Datacenter"}),e.jsx("button",{onClick:()=>o("orgkey"),className:"rounded-md px-3 py-1.5 text-sm font-medium bg-background shadow-sm",children:"Organization Key"})]}),e.jsxs("div",{className:"rounded-lg border border-border bg-card p-4 space-y-4",children:[e.jsxs("div",{className:"flex items-start justify-between gap-3",children:[e.jsxs("div",{children:[e.jsx("h3",{className:"text-base font-semibold",children:"Organization Enrollment Key"}),e.jsx("p",{className:"text-xs text-muted-foreground mt-1",children:"Use one shared isok_ key to install agents on any server in this org."})]}),e.jsx("button",{onClick:()=>k.mutate(),className:"rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700",children:"Generate New Key"})]}),(p||a)&&e.jsxs("div",{className:"rounded-lg border border-border p-3 bg-muted/20 space-y-3",children:[e.jsxs("div",{className:"flex items-center gap-2",children:[e.jsx("span",{className:"font-mono text-sm",children:p||`${a==null?void 0:a.display_prefix}••••••••••`}),p&&e.jsx("button",{onClick:()=>navigator.clipboard.writeText(p),className:"rounded px-2 py-1 text-xs bg-zinc-700 text-zinc-100 hover:bg-zinc-600",children:"Copy"})]}),a&&e.jsxs("div",{className:"flex flex-wrap items-center gap-3 text-xs",children:[e.jsxs("span",{className:"text-muted-foreground",children:["Max registrations/hour: ",e.jsx("span",{className:"text-foreground",children:a.max_registrations_per_hour})]}),e.jsxs("span",{className:"text-muted-foreground",children:["Allowed CIDR: ",e.jsx("span",{className:"text-foreground",children:a.allowed_cidr??"Any"})]}),e.jsxs("label",{className:"inline-flex items-center gap-1 text-muted-foreground",children:[e.jsx("input",{type:"checkbox",checked:a.require_approval,onChange:j=>C.mutate({id:a.id,value:j.target.checked})}),"Require manual approval"]}),e.jsx("button",{onClick:()=>i.mutate(a.id),className:"rounded bg-red-600 px-2 py-1 text-white hover:bg-red-700",children:"Revoke Key"})]})]}),e.jsxs("div",{className:"space-y-2",children:[e.jsx("p",{className:"text-xs font-semibold text-muted-foreground uppercase tracking-wide",children:"Install Steps"}),e.jsxs("ol",{className:"list-decimal list-inside text-sm text-muted-foreground space-y-1",children:[e.jsx("li",{children:"Generate or copy the organization key above."}),e.jsx("li",{children:"Run the install command on each server (same key for all servers)."}),e.jsx("li",{children:"Each server receives a unique isak_ key automatically at registration."}),e.jsx("li",{children:"Continue in this wizard using Register Datacenter tab."})]}),e.jsx(R,{text:g,label:"Quick Install Command"})]}),w.length>0&&e.jsxs("div",{className:"space-y-2",children:[e.jsx("p",{className:"text-xs font-semibold text-muted-foreground uppercase tracking-wide",children:"Recent Registrations"}),e.jsx("div",{className:"rounded-lg border border-border overflow-hidden",children:w.slice(0,5).map(j=>e.jsxs("div",{className:"px-3 py-2 text-xs border-b last:border-b-0 border-border flex items-center justify-between",children:[e.jsx("span",{className:"font-mono text-muted-foreground",children:j.request_ip??"n/a"}),e.jsx("span",{children:j.hostname??"unknown-host"}),e.jsx("span",{className:"text-muted-foreground",children:new Date(j.created_at).toLocaleString()}),e.jsx("span",{className:j.status==="registered"?"text-green-400":"text-yellow-400",children:j.status})]},j.id))})]})]})]})}return h?e.jsxs("div",{className:"space-y-6",children:[e.jsxs("div",{className:"flex gap-1 rounded-lg bg-muted p-1 w-fit",children:[e.jsx("button",{onClick:()=>o("datacenter"),className:"rounded-md px-3 py-1.5 text-sm font-medium bg-background shadow-sm",children:"Register Datacenter"}),e.jsx("button",{onClick:()=>o("orgkey"),className:"rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground hover:text-foreground",children:"Organization Key"})]}),e.jsx("div",{className:"rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-4",children:e.jsxs("div",{className:"flex items-center gap-2 text-yellow-400 font-medium mb-2",children:[e.jsx(te,{className:"h-5 w-5"}),"This key will not be shown again. Copy it now."]})}),e.jsxs("div",{className:"relative",children:[e.jsx("pre",{className:"rounded-lg bg-zinc-900 p-4 pr-12 text-sm font-mono text-green-400 overflow-x-auto",children:h}),e.jsx("button",{onClick:I,className:"absolute top-3 right-3 rounded p-1.5 hover:bg-zinc-700 transition-colors",title:"Copy API key",children:b?e.jsx($,{className:"h-4 w-4 text-green-400"}):e.jsx(Z,{className:"h-4 w-4 text-zinc-400"})})]}),e.jsxs("label",{className:"flex items-center gap-2 text-sm cursor-pointer",children:[e.jsx("input",{type:"checkbox",checked:r,onChange:g=>N(g.target.checked),className:"rounded"}),"I have copied the API key securely"]}),e.jsxs("button",{disabled:!r,onClick:()=>t(h,l),className:"flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors",children:["Next ",e.jsx(T,{className:"h-4 w-4"})]})]}):e.jsxs("div",{className:"space-y-4",children:[e.jsxs("div",{className:"flex gap-1 rounded-lg bg-muted p-1 w-fit",children:[e.jsx("button",{onClick:()=>o("datacenter"),className:"rounded-md px-3 py-1.5 text-sm font-medium bg-background shadow-sm",children:"Register Datacenter"}),e.jsx("button",{onClick:()=>o("orgkey"),className:"rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground hover:text-foreground",children:"Organization Key"})]}),e.jsxs("form",{onSubmit:g=>{g.preventDefault(),y.mutate()},className:"space-y-4 max-w-md",children:[e.jsxs("div",{children:[e.jsx("label",{className:"block text-sm font-medium text-slate-200 mb-1",children:"Datacenter Name"}),e.jsx("input",{value:l,onChange:g=>x(g.target.value),required:!0,placeholder:"e.g. us-east-prod",className:"w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500"})]}),e.jsxs("div",{children:[e.jsx("label",{className:"block text-sm font-medium text-slate-200 mb-1",children:"Region Label (optional)"}),e.jsx("input",{value:m,onChange:g=>c(g.target.value),placeholder:"e.g. US East",className:"w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500"})]}),e.jsxs("button",{type:"submit",disabled:!l.trim()||y.isPending,className:"flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-40 transition-colors",children:[y.isPending?e.jsx(A,{className:"h-4 w-4 animate-spin"}):e.jsx(z,{className:"h-4 w-4"}),"Register"]}),y.isError&&e.jsx("p",{className:"text-sm text-red-400",children:y.error.message})]})]})}function R({text:t,label:s}){const[o,l]=n.useState(!1);return e.jsxs("div",{className:"relative",children:[s&&e.jsx("p",{className:"text-xs text-muted-foreground mb-1.5 font-medium uppercase tracking-wide",children:s}),e.jsx("pre",{className:"rounded-lg bg-zinc-900 p-4 pr-12 text-sm font-mono text-green-400 overflow-x-auto whitespace-pre-wrap",children:t}),e.jsx("button",{onClick:()=>{navigator.clipboard.writeText(t),l(!0),setTimeout(()=>l(!1),2e3)},className:"absolute top-3 right-3 rounded p-1.5 hover:bg-zinc-700 transition-colors",title:"Copy",children:o?e.jsx($,{className:"h-4 w-4 text-green-400"}):e.jsx(Z,{className:"h-4 w-4 text-zinc-400"})})]})}function me({apiKey:t,dcName:s,onNext:o}){const[l,x]=n.useState("linux"),[m,c]=n.useState("agent"),[h,f]=n.useState(!1),b=window.location.origin,u=`${window.location.protocol}//${window.location.hostname}:5006`,r=`${window.location.protocol}//${window.location.hostname}:4317`,N=`curl -fsSL ${b}/dl/zentra-agent-linux-x86_64 -o zentra-agent && \\
chmod +x zentra-agent && \\
ZENTRA_AGENT_MODE=remote \\
ZENTRA_API_ENDPOINT=${u} \\
ZENTRA_AGENT_API_KEY=${t} \\
ZENTRA_DC_NAME=${s} \\
./zentra-agent`,p=`# Install OpenTelemetry Collector (contrib)
curl -fsSL https://github.com/open-telemetry/opentelemetry-collector-releases/releases/latest/download/otelcol-contrib_linux_amd64.tar.gz | tar -xz
chmod +x otelcol-contrib

# Create config.yaml
cat > otel-config.yaml << 'EOF'
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318
  hostmetrics:
    collection_interval: 30s
    scrapers:
      cpu:
      memory:
      disk:
      filesystem:
      load:
      network:
      process:

exporters:
  otlp:
    endpoint: ${r}
    tls:
      insecure: true
    headers:
      Authorization: "Bearer ${t}"
      X-Zentra-DC: "${s}"

service:
  pipelines:
    metrics:
      receivers: [otlp, hostmetrics]
      exporters: [otlp]
    traces:
      receivers: [otlp]
      exporters: [otlp]
    logs:
      receivers: [otlp]
      exporters: [otlp]
EOF

./otelcol-contrib --config=otel-config.yaml`,d=`[Unit]
Description=Zentra Monitoring Agent
After=network.target

[Service]
Type=simple
Environment=ZENTRA_AGENT_MODE=remote
Environment=ZENTRA_API_ENDPOINT=${u}
Environment=ZENTRA_AGENT_API_KEY=${t}
Environment=ZENTRA_DC_NAME=${s}
ExecStart=/usr/local/bin/zentra-agent
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target`,v=`[Unit]
Description=Zentra OTel Collector
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/otelcol-contrib --config=/etc/zentra/otel-config.yaml
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target`,a=`Invoke-WebRequest -Uri "${b}/dl/zentra-agent-windows-x86_64.exe" -OutFile zentra-agent.exe
$env:ZENTRA_AGENT_MODE="remote"
$env:ZENTRA_API_ENDPOINT="${u}"
$env:ZENTRA_AGENT_API_KEY="${t}"
$env:ZENTRA_DC_NAME="${s}"
.\\zentra-agent.exe`,w=`# Download OTel Collector for Windows
Invoke-WebRequest -Uri "https://github.com/open-telemetry/opentelemetry-collector-releases/releases/latest/download/otelcol-contrib_windows_amd64.tar.gz" -OutFile otelcol.tar.gz
tar -xf otelcol.tar.gz

# Create config (save as otel-config.yaml)
$config = @"
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318
  hostmetrics:
    collection_interval: 30s
    scrapers: { cpu: , memory: , disk: , filesystem: , load: , network: }

exporters:
  otlp:
    endpoint: ${r}
    tls:
      insecure: true
    headers:
      Authorization: "Bearer ${t}"
      X-Zentra-DC: "${s}"

service:
  pipelines:
    metrics:
      receivers: [otlp, hostmetrics]
      exporters: [otlp]
"@
$config | Out-File -FilePath otel-config.yaml -Encoding utf8

.\\otelcol-contrib.exe --config=otel-config.yaml`,k=`docker run -d \\
  --name zentra-agent \\
  --restart unless-stopped \\
  --network host \\
  --pid host \\
  -v /proc:/host/proc:ro \\
  -v /sys:/host/sys:ro \\
  -v /etc:/host/etc:ro \\
  -e ZENTRA_AGENT_MODE=remote \\
  -e ZENTRA_API_ENDPOINT=${u} \\
  -e ZENTRA_AGENT_API_KEY=${t} \\
  -e ZENTRA_DC_NAME=${s} \\
  zentraio/agent:latest`,i=`# docker-compose.yaml
version: "3.8"
services:
  otel-collector:
    image: otel/opentelemetry-collector-contrib:latest
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./otel-config.yaml:/etc/otel/config.yaml
    command: ["--config", "/etc/otel/config.yaml"]

# otel-config.yaml
# receivers:
#   otlp:
#     protocols:
#       grpc:
#         endpoint: 0.0.0.0:4317
#   hostmetrics:
#     collection_interval: 30s
#     scrapers: { cpu: , memory: , disk: , filesystem: , load: , network: }
# exporters:
#   otlp:
#     endpoint: ${r}
#     tls:
#       insecure: true
#     headers:
#       Authorization: "Bearer ${t}"
#       X-Zentra-DC: "${s}"
# service:
#   pipelines:
#     metrics:
#       receivers: [otlp, hostmetrics]
#       exporters: [otlp]`,C=`# otel-snmp-config.yaml — monitors network devices via SNMP
# Replace device_ip / community string / SNMP version as needed.
receivers:
  snmp:
    collection_interval: 60s
    endpoint: udp://DEVICE_IP:161
    version: v2c
    community: public
    # For SNMPv3:
    # version: v3
    # user: snmp_user
    # security_level: auth_priv
    # auth_protocol: MD5
    # auth_password: auth_pass
    # privacy_protocol: DES
    # privacy_password: priv_pass
    attributes:
      device_name:
        oid: 1.3.6.1.2.1.1.5.0
        enum:
          - sysName
    metrics:
      # Interface metrics
      network.interface.in.packets:
        unit: packets
        gauge:
          value_type: int
        oid: 1.3.6.1.2.1.2.2.1.11
      network.interface.out.packets:
        unit: packets
        gauge:
          value_type: int
        oid: 1.3.6.1.2.1.2.2.1.17
      network.interface.in.errors:
        unit: errors
        gauge:
          value_type: int
        oid: 1.3.6.1.2.1.2.2.1.14
      network.interface.out.errors:
        unit: errors
        gauge:
          value_type: int
        oid: 1.3.6.1.2.1.2.2.1.20
      # CPU & Memory (Cisco-style)
      system.cpu.utilization:
        unit: percent
        gauge:
          value_type: double
        oid: 1.3.6.1.4.1.9.9.109.1.1.1.1.7.1
      system.memory.utilization:
        unit: percent
        gauge:
          value_type: double
        oid: 1.3.6.1.4.1.9.9.109.1.1.1.1.12.1

exporters:
  otlp:
    endpoint: ${r}
    tls:
      insecure: true
    headers:
      Authorization: "Bearer ${t}"
      X-Zentra-DC: "${s}"

service:
  pipelines:
    metrics:
      receivers: [snmp]
      exporters: [otlp]`,L={linux:{agent:N,collector:p,network:`# 1. Download OTel Collector contrib (includes SNMP receiver)
curl -fsSL https://github.com/open-telemetry/opentelemetry-collector-releases/releases/latest/download/otelcol-contrib_linux_amd64.tar.gz | tar -xz
chmod +x otelcol-contrib

# 2. Save the config below as otel-snmp-config.yaml
#    (edit DEVICE_IP and SNMP credentials first)

# 3. Run the collector
./otelcol-contrib --config=otel-snmp-config.yaml`},windows:{agent:a,collector:w,network:`# 1. Download OTel Collector contrib (includes SNMP receiver)
Invoke-WebRequest -Uri "https://github.com/open-telemetry/opentelemetry-collector-releases/releases/latest/download/otelcol-contrib_windows_amd64.tar.gz" -OutFile otelcol.tar.gz
tar -xf otelcol.tar.gz

# 2. Save the config below as otel-snmp-config.yaml
#    (edit DEVICE_IP and SNMP credentials first)

# 3. Run the collector
.otelcol-contrib.exe --config=otel-snmp-config.yaml`},docker:{agent:k,collector:i,network:`# docker-compose.yaml — SNMP Network Device Collector
version: "3.8"
services:
  snmp-collector:
    image: otel/opentelemetry-collector-contrib:latest
    restart: unless-stopped
    volumes:
      - ./otel-snmp-config.yaml:/etc/otel/config.yaml
    command: ["--config", "/etc/otel/config.yaml"]
    ports:
      - "4317:4317"

# Save otel-snmp-config.yaml next to docker-compose.yaml
# (edit DEVICE_IP and SNMP credentials before running)`}}[l][m];return e.jsxs("div",{className:"space-y-6",children:[e.jsxs("div",{children:[e.jsx("p",{className:"text-xs text-muted-foreground mb-2 font-medium uppercase tracking-wide",children:"Platform"}),e.jsx("div",{className:"flex gap-1 rounded-lg bg-muted p-1 w-fit",children:["linux","windows","docker"].map(_=>e.jsx("button",{onClick:()=>x(_),className:`rounded-md px-4 py-1.5 text-sm font-medium capitalize transition-colors ${l===_?"bg-background shadow-sm":"text-muted-foreground hover:text-foreground"}`,children:_==="docker"?"🐳 Docker":_==="linux"?"🐧 Linux":"🪟 Windows"},_))})]}),e.jsxs("div",{children:[e.jsx("p",{className:"text-xs text-muted-foreground mb-2 font-medium uppercase tracking-wide",children:"Agent Type"}),e.jsxs("div",{className:"flex flex-wrap gap-2",children:[e.jsxs("button",{onClick:()=>c("agent"),className:`flex items-center gap-2 rounded-lg border px-4 py-2.5 text-sm font-medium transition-colors ${m==="agent"?"border-blue-500 bg-blue-500/10 text-blue-400":"border-border text-muted-foreground hover:border-blue-500/50 hover:text-foreground"}`,children:[e.jsx(z,{className:"h-4 w-4"}),e.jsxs("div",{className:"text-left",children:[e.jsx("div",{children:"Zentra Agent"}),e.jsx("div",{className:"text-xs opacity-70",children:"Full monitoring — metrics, logs, discovery"})]})]}),e.jsxs("button",{onClick:()=>c("collector"),className:`flex items-center gap-2 rounded-lg border px-4 py-2.5 text-sm font-medium transition-colors ${m==="collector"?"border-purple-500 bg-purple-500/10 text-purple-400":"border-border text-muted-foreground hover:border-purple-500/50 hover:text-foreground"}`,children:[e.jsx(F,{className:"h-4 w-4"}),e.jsxs("div",{className:"text-left",children:[e.jsx("div",{children:"OTel Collector Agent"}),e.jsx("div",{className:"text-xs opacity-70",children:"OTLP pipeline — traces, metrics, logs"})]})]}),e.jsxs("button",{onClick:()=>c("network"),className:`flex items-center gap-2 rounded-lg border px-4 py-2.5 text-sm font-medium transition-colors ${m==="network"?"border-cyan-500 bg-cyan-500/10 text-cyan-400":"border-border text-muted-foreground hover:border-cyan-500/50 hover:text-foreground"}`,children:[e.jsx(O,{className:"h-4 w-4"}),e.jsxs("div",{className:"text-left",children:[e.jsx("div",{children:"Network Device Agent"}),e.jsx("div",{className:"text-xs opacity-70",children:"SNMP polling — switches, routers, firewalls"})]})]})]})]}),e.jsx(R,{text:L}),m==="network"&&e.jsxs("div",{className:"space-y-2",children:[e.jsxs("div",{className:"flex items-center gap-2 text-xs text-cyan-400 font-medium uppercase tracking-wide",children:[e.jsx(O,{className:"h-3.5 w-3.5"}),"SNMP Collector Config (otel-snmp-config.yaml)"]}),e.jsxs("p",{className:"text-xs text-muted-foreground",children:["Edit ",e.jsx("code",{className:"text-cyan-400",children:"DEVICE_IP"})," and SNMP credentials before starting the collector."]}),e.jsx(R,{text:C})]}),l==="linux"&&e.jsxs("div",{children:[e.jsxs("button",{onClick:()=>f(!h),className:"flex items-center gap-1.5 text-sm text-blue-400 hover:text-blue-300 transition-colors",children:[h?e.jsx(q,{className:"h-4 w-4"}):e.jsx(T,{className:"h-4 w-4"}),"systemd service file (persistent background service)"]}),h&&e.jsx("div",{className:"mt-2",children:e.jsx(R,{text:m==="agent"?d:v})})]}),e.jsxs("button",{onClick:o,className:"flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors",children:["I've run the command ",e.jsx(T,{className:"h-4 w-4"})]})]})}function xe({dcName:t,onConnected:s}){const o=n.useRef(Date.now()),[l,x]=n.useState(!1),{data:m}=S({queryKey:["agents","poll"],queryFn:H,refetchInterval:5e3});return n.useEffect(()=>{const c=setTimeout(()=>x(!0),3e5);return()=>clearTimeout(c)},[]),n.useEffect(()=>{m!=null&&m.some(c=>c.datacenter_name===t&&c.status==="active")&&s()},[m,t,s]),l?e.jsxs("div",{className:"space-y-4",children:[e.jsxs("div",{className:"rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-4 space-y-3",children:[e.jsx("h3",{className:"font-medium text-yellow-400",children:"Connection not detected after 5 minutes"}),e.jsx("p",{className:"text-sm text-muted-foreground",children:"Troubleshooting checklist:"}),e.jsxs("ul",{className:"list-disc list-inside text-sm text-muted-foreground space-y-1",children:[e.jsxs("li",{children:["Verify the server can reach ",e.jsx("code",{className:"text-xs",children:window.location.origin})," over HTTPS"]}),e.jsxs("li",{children:["Check the API key was copied correctly (starts with ",e.jsx("code",{className:"text-xs",children:"isok_"}),")"]}),e.jsx("li",{children:"Ensure no firewall is blocking outbound port 443"}),e.jsxs("li",{children:["Check the agent process is running: ",e.jsx("code",{className:"text-xs",children:"ps aux | grep zentra-agent"})]}),e.jsx("li",{children:"Inspect the live agent log tail below for errors"})]})]}),e.jsx(P,{dcName:t}),e.jsx("button",{onClick:()=>{x(!1),o.current=Date.now()},className:"rounded-lg bg-zinc-700 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-600",children:"Retry"})]}):e.jsxs("div",{className:"space-y-6",children:[e.jsxs("div",{className:"flex flex-col items-center justify-center py-8 space-y-3",children:[e.jsx(A,{className:"h-10 w-10 animate-spin text-blue-400"}),e.jsx("p",{className:"text-muted-foreground",children:"Waiting for agent to connect…"}),e.jsx("p",{className:"text-xs text-muted-foreground",children:"Polling every 5 seconds"})]}),e.jsx(P,{dcName:t})]})}function ue(t){switch(t){case"oracle":return e.jsx(le,{className:"h-4 w-4 text-blue-400"});case"nginx":case"apache":case"haproxy":case"tomcat":return e.jsx(ae,{className:"h-4 w-4 text-green-400"});case"redis":case"memcached":case"kafka":case"rabbitmq":case"elasticsearch":return e.jsx(F,{className:"h-4 w-4 text-amber-400"});case"network_interface":return e.jsx(oe,{className:"h-4 w-4 text-cyan-400"});case"log_directory":return e.jsx(ne,{className:"h-4 w-4 text-yellow-400"});case"linux":case"server":return e.jsx(z,{className:"h-4 w-4 text-zinc-400"});default:return e.jsx(re,{className:"h-4 w-4 text-zinc-400"})}}function pe({comp:t,selected:s,onToggle:o}){const l=t.confirmed;return e.jsxs("tr",{className:`border-b border-border last:border-0 transition-colors ${l?"opacity-60":s?"bg-blue-500/5":"hover:bg-muted/30"}`,children:[e.jsx("td",{className:"px-4 py-3",children:l?e.jsx(D,{className:"h-4 w-4 text-green-400"}):e.jsx("input",{type:"checkbox",checked:s,onChange:o,className:"rounded border-border"})}),e.jsx("td",{className:"px-4 py-3",children:e.jsxs("div",{className:"flex items-center gap-2",children:[ue(t.component_type),e.jsx("span",{className:"font-medium",children:t.component_name}),l&&e.jsx("span",{className:"rounded-full bg-green-500/15 px-2 py-0.5 text-xs text-green-400",children:"Added"})]})}),e.jsx("td",{className:"px-4 py-3",children:e.jsx("span",{className:"rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground",children:t.component_type})}),e.jsxs("td",{className:"px-4 py-3 font-mono text-xs text-muted-foreground",children:[t.address,t.port?`:${t.port}`:""]}),e.jsx("td",{className:"px-4 py-3 text-xs text-muted-foreground",children:t.version??"—"})]})}function ge({dcName:t}){const s=G(),[o,l]=n.useState(new Set),[x,m]=n.useState(!1),[c,h]=n.useState(0),{data:f,isLoading:b,isError:u,refetch:r,isFetching:N}=S({queryKey:["components",t],queryFn:()=>J(t),refetchInterval:x?!1:15e3,enabled:!!t}),p=(f==null?void 0:f.components)??[],d=p.filter(i=>!i.confirmed),v=p.filter(i=>i.confirmed);n.useEffect(()=>{d.length>0&&o.size===0&&l(new Set(d.map(i=>i.id)))},[d.length]);const a=E({mutationFn:()=>V({component_ids:Array.from(o),datacenter_name:t}),onSuccess:i=>{h(i.created),m(!0),r()}}),w=()=>{o.size===d.length?l(new Set):l(new Set(d.map(i=>i.id)))},k=i=>{l(C=>{const y=new Set(C);return y.has(i)?y.delete(i):y.add(i),y})};return e.jsxs("div",{className:"space-y-6",children:[e.jsxs("div",{className:"flex items-center gap-3 text-green-400",children:[e.jsx(D,{className:"h-8 w-8"}),e.jsx("h3",{className:"text-lg font-semibold",children:"Agent connected successfully!"})]}),x&&c>0&&e.jsx("div",{className:"rounded-lg border border-green-500/30 bg-green-500/10 p-4",children:e.jsxs("p",{className:"text-sm text-green-400 font-medium",children:["✓ ",c," monitoring target",c!==1?"s":""," created and will start collecting shortly."]})}),b?e.jsxs("div",{className:"flex flex-col items-center justify-center py-10 gap-3",children:[e.jsx(A,{className:"h-8 w-8 animate-spin text-blue-400"}),e.jsx("p",{className:"text-sm text-muted-foreground",children:"Waiting for agent to report discovered components…"}),e.jsx("p",{className:"text-xs text-muted-foreground",children:"This may take up to 30 seconds after the agent starts."})]}):u?e.jsxs("div",{className:"rounded-lg border border-red-500/30 bg-red-500/10 p-4 space-y-3",children:[e.jsx("p",{className:"text-sm font-medium text-red-400",children:"Failed to fetch discovered components"}),e.jsx("p",{className:"text-xs text-muted-foreground",children:"Possible reasons: agent not yet connected, discovery still running, or network issue."}),e.jsxs("button",{onClick:()=>r(),className:"flex items-center gap-2 rounded-lg bg-zinc-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-zinc-600",children:[e.jsx(M,{className:"h-3.5 w-3.5"})," Retry"]})]}):p.length===0?e.jsxs("div",{className:"rounded-lg border border-border bg-card p-6 space-y-4",children:[e.jsxs("div",{className:"text-center space-y-2",children:[e.jsx("p",{className:"text-muted-foreground font-medium",children:"No components discovered yet"}),e.jsx("p",{className:"text-xs text-muted-foreground",children:"The agent scans for local services every 5 minutes."})]}),e.jsxs("ul",{className:"list-disc list-inside text-xs text-muted-foreground space-y-1",children:[e.jsx("li",{children:"Ensure the agent process is running on the target host"}),e.jsxs("li",{children:["Agent must be able to reach ",e.jsx("code",{className:"text-xs",children:window.location.origin})]}),e.jsx("li",{children:"Check agent logs below for errors"}),e.jsx("li",{children:"Supported: PostgreSQL, MySQL, Redis, nginx, Apache, MongoDB, Oracle, Kafka, RabbitMQ"})]}),e.jsxs("button",{onClick:()=>r(),disabled:N,className:"flex items-center gap-2 rounded-lg bg-zinc-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-zinc-600 disabled:opacity-50",children:[N?e.jsx(A,{className:"h-3.5 w-3.5 animate-spin"}):e.jsx(M,{className:"h-3.5 w-3.5"}),"Retry Discovery"]})]}):e.jsxs("div",{className:"space-y-4",children:[v.length>0&&e.jsxs("p",{className:"text-xs text-muted-foreground",children:[v.length," component",v.length!==1?"s":""," already added as targets."]}),e.jsx("div",{className:"rounded-xl border border-border bg-card overflow-hidden",children:e.jsxs("table",{className:"w-full text-sm",children:[e.jsx("thead",{children:e.jsxs("tr",{className:"border-b border-border text-left text-muted-foreground bg-muted/30",children:[e.jsx("th",{className:"px-4 py-3 w-10",children:d.length>0&&e.jsx("input",{type:"checkbox",checked:o.size===d.length&&d.length>0,onChange:w,className:"rounded border-border"})}),e.jsx("th",{className:"px-4 py-3 font-medium",children:"Component"}),e.jsx("th",{className:"px-4 py-3 font-medium",children:"Type"}),e.jsx("th",{className:"px-4 py-3 font-medium",children:"Address"}),e.jsx("th",{className:"px-4 py-3 font-medium",children:"Version"})]})}),e.jsx("tbody",{children:p.map(i=>e.jsx(pe,{comp:i,selected:o.has(i.id),onToggle:()=>k(i.id)},i.id))})]})}),d.length>0&&!x&&e.jsxs("div",{className:"flex items-center gap-4",children:[e.jsxs("button",{onClick:()=>a.mutate(),disabled:o.size===0||a.isPending,className:"flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors",children:[a.isPending?e.jsx(A,{className:"h-4 w-4 animate-spin"}):e.jsx(D,{className:"h-4 w-4"}),"Add ",o.size," selected component",o.size!==1?"s":""," to monitoring"]}),a.isError&&e.jsx("p",{className:"text-sm text-red-400",children:a.error.message})]})]}),e.jsx(P,{dcName:t,defaultOpen:!1}),e.jsxs("div",{className:"flex items-center gap-3",children:[e.jsxs("button",{onClick:()=>s("/dashboard"),className:"flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors",children:["Go to Dashboard ",e.jsx(T,{className:"h-4 w-4"})]}),e.jsx("button",{onClick:()=>s("/servers"),className:"flex items-center gap-2 rounded-lg bg-zinc-700 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-600 transition-colors",children:"Server Fleet"})]})]})}function we(){const[t,s]=n.useState(1),[o,l]=n.useState(""),[x,m]=n.useState(""),c=n.useCallback((h,f)=>{l(h),m(f),s(2)},[]);return e.jsxs("div",{className:"space-y-6",children:[e.jsx(ee,{title:"Install Agent",description:"Connect a datacenter in four quick steps.",icon:z}),e.jsx("div",{className:"flex items-center gap-2",children:ie.map((h,f)=>{const b=f+1,u=b===t,r=b<t;return e.jsxs("div",{className:"flex items-center gap-2",children:[f>0&&e.jsx("div",{className:`h-px w-8 ${r?"bg-blue-500":"bg-border"}`}),e.jsxs("div",{className:"flex items-center gap-1.5",children:[e.jsx("div",{className:`flex h-7 w-7 items-center justify-center rounded-full text-xs font-bold transition-colors ${r?"bg-blue-600 text-white":u?"bg-blue-600 text-white ring-2 ring-blue-400/50":"bg-muted text-muted-foreground"}`,children:r?e.jsx($,{className:"h-3.5 w-3.5"}):b}),e.jsx("span",{className:`text-xs hidden sm:inline ${u?"font-medium text-foreground":"text-muted-foreground"}`,children:h})]})]},b)})}),e.jsxs("div",{className:"rounded-xl border border-border bg-card p-6",children:[t===1&&e.jsx(de,{onComplete:c}),t===2&&e.jsx(me,{apiKey:o,dcName:x,onNext:()=>s(3)}),t===3&&e.jsx(xe,{dcName:x,onConnected:()=>s(4)}),t===4&&e.jsx(ge,{dcName:x})]})]})}export{we as default};
