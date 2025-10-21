#!/usr/bin/env python3
import sys, os, json, ssl, socket, urllib.request, urllib.parse, time

# ==== KONFIG INFLUXDB v2 ====
INFLUX_URL    = os.getenv("INFLUX_URL", "http://localhost:8086").rstrip("/")
INFLUX_TOKEN  = os.getenv("INFLUX_TOKEN", "ZISIdQAqG9EPJK_mgaW5q0IfNl5-M1npTE5-ouPpDiP5PnxHchCTgAFdE1g2YFa5OuoRCd9U7LFVWGmV9IomgQ==")
INFLUX_ORG    = os.getenv("INFLUX_ORG", "ITS")
INFLUX_BUCKET = os.getenv("INFLUX_BUCKET", "skt13esp")

# ==== Kunci JSON yang dikirim ESP (harus sama dengan main.rs) ====
MEASUREMENT   = os.getenv("MEASUREMENT",  "sht20")
FIELD_TEMP    = os.getenv("FIELD_TEMP",   "esp32s3 Temperature")
FIELD_HUM     = os.getenv("FIELD_HUM",    "esp32s3 Humidity")
FIELD_SENSOR  = os.getenv("FIELD_SENSOR", "sensor")
TS_FIELD      = os.getenv("TS_FIELD",     "ts_ms")  # epoch ms

# ==== Socket/Keepalive ====
CONNECT_TIMEOUT_S   = int(os.getenv("CONNECT_TIMEOUT_S", "10"))
READ_TIMEOUT_S      = int(os.getenv("READ_TIMEOUT_S", "150"))  # > 60 dtk (interval kirim)
RECONNECT_DELAY_S   = int(os.getenv("RECONNECT_DELAY_S", "2"))

KA_ENABLE           = int(os.getenv("KA_ENABLE", "1"))
KA_IDLE_S           = int(os.getenv("KA_IDLE_S", "45"))
KA_INTVL_S          = int(os.getenv("KA_INTVL_S", "15"))
KA_CNT              = int(os.getenv("KA_CNT", "3"))

def log(msg): print(msg, flush=True)

def _esc_tag_val(v: str) -> str:
    # tag VALUE escape: koma, spasi, '='
    return v.replace(",", r"\,").replace(" ", r"\ ").replace("=", r"\=")

def _esc_field_key(k: str) -> str:
    # field KEY escape: koma, spasi, '='
    return k.replace(",", r"\,").replace(" ", r"\ ").replace("=", r"\=")

def _maybe_tags(sensor: str) -> str:
    # kembalikan "" (tanpa koma) kalau tidak ada tag
    if sensor:
        return "sensor=" + _esc_tag_val(sensor)
    return ""

def build_line_protocol(sensor: str, t_c: float, h_pct: float, ts_ms: int | None):
    tagpart = _maybe_tags(sensor)
    # field keys harus di-escape
    f1 = f"{_esc_field_key(FIELD_TEMP)}={float(t_c)}"
    f2 = f"{_esc_field_key(FIELD_HUM)}={float(h_pct)}"
    fieldpart = f"{f1},{f2}"

    # measurement + (opsional) tags
    head = MEASUREMENT if tagpart == "" else f"{MEASUREMENT},{tagpart}"

    if ts_ms is None:
        # presisi 's' → timestamp server
        return f"{head} {fieldpart}", "s"
    else:
        # presisi 'ms' → pakai timestamp dari perangkat
        return f"{head} {fieldpart} {int(ts_ms)}", "ms"

def influx_write(lines: str, precision: str):
    if not INFLUX_TOKEN: raise RuntimeError("INFLUX_TOKEN kosong")
    url = f"{INFLUX_URL}/api/v2/write?" + urllib.parse.urlencode({
        "org": INFLUX_ORG, "bucket": INFLUX_BUCKET, "precision": precision
    })
    req = urllib.request.Request(
        url, data=lines.encode("utf-8"), method="POST",
        headers={
            "Authorization": f"Token {INFLUX_TOKEN}",
            "Content-Type": "text/plain; charset=utf-8",
            "Accept": "application/json",
        }
    )
    ctx = ssl.create_default_context() if url.startswith("https://") else None
    try:
        with urllib.request.urlopen(req, context=ctx, timeout=15) as resp:
            if resp.status not in (200, 204):
                body = resp.read().decode("utf-8", "ignore")
                raise RuntimeError(f"Influx write gagal: HTTP {resp.status} {body}")
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", "ignore")
        raise RuntimeError(f"Influx write gagal: HTTP {e.code} {body}")

def handle_line(line: str):
    obj = json.loads(line)
    if FIELD_TEMP not in obj or FIELD_HUM not in obj:
        raise ValueError(f"JSON harus punya '{FIELD_TEMP}' dan '{FIELD_HUM}'")

    sensor = str(obj.get(FIELD_SENSOR, "")).strip()
    t_c    = float(obj[FIELD_TEMP])
    h_pct  = float(obj[FIELD_HUM])

    ts_ms  = obj.get(TS_FIELD)
    # Validasi epoch ms (>= ~2017)
    if isinstance(ts_ms, (int, float)) and int(ts_ms) >= 1_500_000_000_000:
        ts_ms = int(ts_ms)
    else:
        ts_ms = None  # biar Influx pakai timestamp server

    lp, prec = build_line_protocol(sensor, t_c, h_pct, ts_ms)
    influx_write(lp, precision=prec)
    log(f"[OK] {sensor or '-'} T={t_c:.3f}°C RH={h_pct:.2f}% ts={'now' if ts_ms is None else ts_ms}")

def _apply_keepalive(sock: socket.socket):
    if not KA_ENABLE: return
    try:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
        if hasattr(socket, "TCP_KEEPIDLE"):
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPIDLE, KA_IDLE_S)
        if hasattr(socket, "TCP_KEEPINTVL"):
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPINTVL, KA_INTVL_S)
        if hasattr(socket, "TCP_KEEPCNT"):
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPCNT, KA_CNT)
    except Exception:
        pass

def run_client(host: str, port: int):
    log(f"[CLIENT] connecting to {host}:{port} ...")
    while True:
        try:
            with socket.create_connection((host, port), timeout=CONNECT_TIMEOUT_S) as sock:
                _apply_keepalive(sock)
                sock.settimeout(READ_TIMEOUT_S)
                log("[CLIENT] connected (keepalive ON, waiting ~60s interval)")
                f = sock.makefile(mode="r", encoding="utf-8", newline="\n")
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        handle_line(line)
                    except Exception as e:
                        log(f"[ERR] handle_line: {e} | RAW: {line}")
                log("[CLIENT] EOF dari server (reconnect)")
        except socket.timeout:
            log("[CLIENT] read timeout (no data). reconnect…")
        except Exception as e:
            log(f"[CLIENT] connect/read error: {e} (retry in {RECONNECT_DELAY_S}s)")
        try:
            time.sleep(RECONNECT_DELAY_S)
        except KeyboardInterrupt:
            break

def main():
    if len(sys.argv) < 3:
        print("usage: influx_tcp_client.py <ESP_IP> <PORT>", file=sys.stderr)
        sys.exit(1)
    host = sys.argv[1]
    port = int(sys.argv[2])
    run_client(host, port)

if __name__ == "__main__":
    main()
