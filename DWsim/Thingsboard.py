#!/usr/bin/env python3
# InfluxDB v2 -> ThingsBoard (MQTT Basic), JSON-query mode
# Mendukung multi-ThingsBoard (TB1 + TB2) secara bersamaan.

import os, time, json, csv, ssl, re
import urllib.request, urllib.parse
from datetime import datetime, timezone
from typing import List, Tuple, Optional
from urllib.parse import urlparse

# ===== Influx v2 =====
INFLUX_URL     = os.getenv("INFLUX_URL", "http://localhost:8086").rstrip("/")
INFLUX_TOKEN   = os.getenv("INFLUX_TOKEN", "CnwVt5c9OvNVJskPQ9W9QPMge6wWZyX4Gfdj8L_Yo77wbJJDuYDIScYL51jioxcXxaFRaPp_IAIu44l-tsZX8A==")
INFLUX_ORG     = os.getenv("INFLUX_ORG", "ITS")
INFLUX_BUCKET  = os.getenv("INFLUX_BUCKET", "skt13dwsim")

MEASUREMENT    = os.getenv("MEASUREMENT", "water_temperature_c")
FIELD          = os.getenv("FIELD", "value")
TAG_FILTERS    = os.getenv("TAG_FILTERS", "stream=water_temp")   
QUERY_LOOKBACK = os.getenv("QUERY_LOOKBACK", "1h")

# ===== ThingsBoard =====
TB_MQTT_URL    = os.getenv("TB_MQTT_URL", "")               
TB_MQTT_HOST   = os.getenv("TB_MQTT_HOST", "thingsboard.cloud")
TB_MQTT_PORT   = int(os.getenv("TB_MQTT_PORT", "1883"))
TB_CLIENT_ID   = os.getenv("TB_CLIENT_ID", "esp32s3-eggdhis")
TB_USERNAME    = os.getenv("TB_USERNAME", "esp32s3SKT13")
TB_PASSWORD    = os.getenv("TB_PASSWORD", "453621")
TB_TOPIC       = os.getenv("TB_TOPIC", "v1/devices/me/telemetry")
TB_KEY         = os.getenv("TB_KEY", "DWsim Temperature")
TB_MQTT_TLS    = int(os.getenv("TB_MQTT_TLS", "0"))                

POLL_INTERVAL_S = int(os.getenv("POLL_INTERVAL_S", "60"))
STATE_FILE      = os.getenv("STATE_FILE", os.path.join(os.path.dirname(__file__), ".influx_tb_mqtt_state.json"))
DEBUG           = bool(int(os.getenv("DEBUG", "0")))

from paho.mqtt import client as mqtt


# ===================== Util / Validasi =====================
def _ensure_cfg():
    missing = []
    if not INFLUX_TOKEN:  missing.append("INFLUX_TOKEN")
    if not INFLUX_ORG:    missing.append("INFLUX_ORG")
    if not INFLUX_BUCKET: missing.append("INFLUX_BUCKET")
    if missing:
        raise SystemExit(f"Wajib set: {', '.join(missing)}")


def _load_state() -> dict:
    try:
        with open(STATE_FILE, "r") as f:
            return json.load(f)
    except Exception:
        return {}


def _save_state(state: dict):
    try:
        with open(STATE_FILE, "w") as f:
            json.dump(state, f)
    except Exception as e:
        print(f"[WARN] save state: {e}")


def _is_duration(s: str) -> bool:
    """
    Cek apakah string adalah durasi relatif valid untuk Flux range(start: …).
    Contoh valid: -1h, -30m, -2d, -15s, -1w
    """
    s = s.strip().lower()
    return bool(re.fullmatch(r"-\d+[smhdw]", s))


def _range_start_clause(start_expr: str) -> str:
    """Kembalikan teks untuk range(start: …) yang valid di Flux."""
    s = start_expr.strip()
    if _is_duration(s):
        return s  # tanpa kutip untuk durasi relatif
    # asumsikan RFC3339 absolut
    return f'time(v: "{s}")'


def _build_flux(start_rfc3339_or_duration: str) -> str:
    filters = [
        f'r["_measurement"] == "{MEASUREMENT}"',
        f'r["_field"] == "{FIELD}"'
    ]
    if TAG_FILTERS:
        for pair in TAG_FILTERS.split(","):
            if "=" in pair:
                k, v = pair.split("=", 1)
                k, v = k.strip(), v.strip()
                if k:
                    filters.append(f'r["{k}"] == "{v}"')
    flt = " and ".join(filters)
    start_clause = _range_start_clause(start_rfc3339_or_duration)

    flux = f'''from(bucket: "{INFLUX_BUCKET}")
  |> range(start: {start_clause})
  |> filter(fn: (r) => {flt})
  |> keep(columns: ["_time","_value"])
  |> sort(columns: ["_time"], desc: false)'''
    return flux


def _http_post(url: str, headers: dict, data: bytes, timeout: int = 25):
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    ctx = None
    if url.startswith("https://"):
        ctx = ssl.create_default_context()
    try:
        with urllib.request.urlopen(req, context=ctx, timeout=timeout) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()
    except Exception as e:
        raise RuntimeError(f"HTTP POST error {url}: {e}")


def query_influx_since(start_expr: str) -> List[Tuple[int, float]]:
    """Return list (ts_ms, value)"""
    flux = _build_flux(start_expr)
    if DEBUG:
        print("----- FLUX -----")
        print(flux)
        print("----------------")

    url = f"{INFLUX_URL}/api/v2/query?org={urllib.parse.quote(INFLUX_ORG)}"
    body = json.dumps({"query": flux, "type": "flux"}).encode("utf-8")
    headers = {
        "Authorization": f"Token {INFLUX_TOKEN}",
        "Content-Type": "application/json",
        "Accept": "application/csv"
    }
    status, resp = _http_post(url, headers, body)
    if status not in (200, 204):
        text = resp.decode("utf-8", "ignore")
        raise RuntimeError(f"Query Influx error HTTP {status}: {text}")

    rows: List[Tuple[int, float]] = []
    text = resp.decode("utf-8", "replace")
    reader = csv.DictReader(text.splitlines())
    for r in reader:
        if "_time" in r and "_value" in r:
            try:
                dt = datetime.fromisoformat(r["_time"].replace("Z", "+00:00"))
                ts_ms = int(dt.timestamp() * 1000)
                val = float(r["_value"])
                rows.append((ts_ms, val))
            except Exception:
                continue
    return rows


# ===================== MQTT ThingsBoard =====================
class TBMqtt:
    def __init__(self, host: str, port: int, username: str, password: str,
                 client_id: str, topic: str, key_name: str, use_tls: bool):
        # Paho <2.0: clean_session=True
        self.client = mqtt.Client(client_id=client_id, clean_session=True)
        # ThingsBoard (access token) → username=token, password=""
        self.client.username_pw_set(username, password)
        self.client.on_connect = self.on_connect
        self.client.on_disconnect = self.on_disconnect
        self.connected = False

        self.host = host
        self.port = port
        self.use_tls = use_tls

        self.topic = topic
        self.key_name = key_name

        if self.use_tls:
            # default CA system. Bisa custom: tls_set(ca_certs="path/to/ca.pem")
            self.client.tls_set()

    def on_connect(self, client, userdata, flags, rc):
        self.connected = (rc == 0)
        print(f"[MQTT {self.host}] {'Connected' if self.connected else f'Connect failed rc={rc}'}")

    def on_disconnect(self, client, userdata, rc):
        self.connected = False
        print(f"[MQTT {self.host}] Disconnected rc={rc}")

    def connect(self):
        self.client.connect(self.host, self.port, keepalive=60)
        self.client.loop_start()
        for _ in range(50):
            if self.connected:
                return
            time.sleep(0.1)
        raise RuntimeError(f"MQTT {self.host} tidak tersambung")

    def ensure_connected(self):
        if not self.connected:
            try:
                self.client.reconnect()
            except Exception:
                self.connect()

    def publish_kv(self, ts_ms: int, value: float):
        """Kirim telemetry satu key (self.key_name) ke self.topic."""
        self.ensure_connected()
        payload = json.dumps({"ts": ts_ms, "values": {self.key_name: value}}, separators=(",", ":"))
        r = self.client.publish(self.topic, payload, qos=1, retain=False)
        if r.rc != mqtt.MQTT_ERR_SUCCESS:
            raise RuntimeError(f"MQTT publish {self.host} rc={r.rc}")


class MultiTB:
    """Fan-out publish ke beberapa ThingsBoard sekaligus."""
    def __init__(self, clients: list[TBMqtt]):
        self.clients = clients

    def connect_all(self):
        for c in self.clients:
            try:
                c.connect()
            except Exception as e:
                print(f"[WARN] connect {c.host}: {e}")

    def publish_all(self, ts_ms: int, value: float):
        errs = 0
        for c in self.clients:
            try:
                c.publish_kv(ts_ms, value)
            except Exception as e:
                errs += 1
                print(f"[ERR ] publish {c.host}: {e}")
        if errs == len(self.clients):
            raise RuntimeError("Semua publish ThingsBoard gagal")


# ===================== Resolver URL/Host/Port/TLS =====================
def _resolve_host_port_tls(url_str: str, fallback_host: str, fallback_port: int, fallback_tls: Optional[int]) -> tuple[str, int, bool]:
    host = fallback_host
    port = fallback_port
    use_tls = bool(fallback_tls) if fallback_tls is not None else False

    if url_str:
        u = urlparse(url_str)
        if not u.hostname:
            raise SystemExit(f"MQTT URL tidak valid: {url_str}")
        host = u.hostname
        if u.port:
            port = int(u.port)
        scheme = (u.scheme or "").lower()
        if scheme == "mqtts":
            use_tls = True
        elif scheme == "mqtt":
            if fallback_tls is None:
                use_tls = False

    if port == 8883 and not use_tls:
        use_tls = True

    return host, port, use_tls


def _build_tb_clients() -> MultiTB:
    # TB1
    host1, port1, tls1 = _resolve_host_port_tls(TB_MQTT_URL, TB_MQTT_HOST, TB_MQTT_PORT, TB_MQTT_TLS)
    tb1 = TBMqtt(
        host=host1,
        port=port1,
        username=TB_USERNAME,
        password=TB_PASSWORD,
        client_id=TB_CLIENT_ID,
        topic=TB_TOPIC,
        key_name=TB_KEY,
        use_tls=tls1
    )

    clients = [tb1]


    mux = MultiTB(clients)
    # Log ringkas
    for c in clients:
        print(f"[MQTT] target {c.host}:{c.port} tls={'on' if c.use_tls else 'off'} topic='{c.topic}' key='{c.key_name}'")
    return mux


# ===================== MAIN =====================
def main():
    _ensure_cfg()

    print(f"[START] Influx -> TB MQTT (multi) | bucket='{INFLUX_BUCKET}' meas='{MEASUREMENT}' field='{FIELD}' key='{TB_KEY}' interval={POLL_INTERVAL_S}s")
    state = _load_state()
    start_ts = state.get("last_ts_rfc3339") or f"-{QUERY_LOOKBACK}"
    print(f"[INIT ] Mulai query dari: {start_ts}")

    mux = _build_tb_clients()
    mux.connect_all()

    while True:
        try:
            rows = query_influx_since(start_ts)
            if rows:
                sent = 0
                max_ts: Optional[int] = None
                for ts_ms, val in rows:
                    mux.publish_all(ts_ms, val)
                    sent += 1
                    if max_ts is None or ts_ms > max_ts:
                        max_ts = ts_ms
                print(f"[OK   ] MQTT kirim {sent} titik. last_ts={max_ts}")
                if max_ts is not None:
                    dt = datetime.fromtimestamp(max_ts / 1000, tz=timezone.utc)
                    start_ts = dt.isoformat()
                    state["last_ts_rfc3339"] = start_ts
                    _save_state(state)
            else:
                print("[INFO ] Tidak ada data baru.")
        except Exception as e:
            print(f"[ERR  ] Loop error: {e}")
        time.sleep(POLL_INTERVAL_S)


if __name__ == "__main__":
    main()
