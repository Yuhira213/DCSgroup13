#!/usr/bin/env python3
import os, io, time, zipfile, ssl, re
import urllib.request, urllib.parse
import xml.etree.ElementTree as ET
from pathlib import Path
from datetime import datetime, timezone, timedelta  # ⬅️ WIB boundary

# ===== InfluxDB v2 =====
INFLUX_URL    = os.getenv("INFLUX_URL", "http://localhost:8086")
INFLUX_TOKEN  = os.getenv("INFLUX_TOKEN", "CnwVt5c9OvNVJskPQ9W9QPMge6wWZyX4Gfdj8L_Yo77wbJJDuYDIScYL51jioxcXxaFRaPp_IAIu44l-tsZX8A==")
INFLUX_ORG    = os.getenv("INFLUX_ORG", "ITS")
INFLUX_BUCKET = os.getenv("INFLUX_BUCKET", "skt13dwsim")

# ===== Sumber data =====
DEFAULT_PATH  = os.path.expanduser("~/bchain/SKT13DWsim.dwxmz")

# ===== Zona Waktu (WIB default) =====
TZ_OFFSET_HOURS = int(os.getenv("TZ_OFFSET_HOURS", "7"))  # WIB = +7
WIB = timezone(timedelta(hours=TZ_OFFSET_HOURS))

# ====== UTIL XML ======
def _load_xml_bytes(path: str) -> bytes:
    p = Path(path)
    if not p.exists():
        raise FileNotFoundError(f"File tidak ditemukan: {p}")
    if p.suffix.lower() == ".dwxmz":
        with zipfile.ZipFile(p, "r") as zf:
            xml_names = [n for n in zf.namelist() if n.lower().endswith(".xml")]
            if not xml_names:
                raise ValueError("Tidak ada .xml di dalam .dwxmz")
            main_xml = sorted(xml_names, key=lambda n: zf.getinfo(n).file_size, reverse=True)[0]
            return zf.read(main_xml)
    return p.read_bytes()

def _find_water_in_object_id(root: ET.Element) -> str | None:
    gos = root.find("GraphicObjects")
    if gos is None:
        return None
    pattern = re.compile(r"water[_\s-]*i(n)?$", re.I)
    for go in gos.findall("GraphicObject"):
        tag = (go.findtext("Tag") or "").strip()
        name = (go.findtext("Name") or "").strip()
        if tag and pattern.search(tag):
            return name or None
    return None

def read_water_in_temp_c(xml_bytes: bytes) -> float | None:
    tree = ET.parse(io.BytesIO(xml_bytes))
    root = tree.getroot()

    target_obj_id = _find_water_in_object_id(root)
    if not target_obj_id:
        return None

    sim_objs_parent = root.find("SimulationObjects")
    if sim_objs_parent is None:
        return None

    for sim_obj in sim_objs_parent.findall("SimulationObject"):
        obj_name = (sim_obj.findtext("Name") or sim_obj.findtext("ComponentName") or "").strip()
        if obj_name != target_obj_id:
            continue

        phases = sim_obj.find("Phases")
        if phases is None:
            continue

        for phase in phases.findall("Phase"):
            phase_name = (phase.findtext("ComponentName") or phase.findtext("Name") or phase.findtext("ID") or "")
            if str(phase_name).lower() == "mixture":
                tK = phase.findtext("Properties/temperature")
                if tK is None:
                    return None
                try:
                    tK = float(tK)
                except ValueError:
                    return None
                return tK - 273.15
    return None

# ====== Influx write ======
def _ssl_context_for(url: str):
    if url.startswith("https://"):
        return ssl.create_default_context()
    return None

def write_to_influx_celsius(measurement: str, field_value_c: float, tags: dict[str, str] | None = None):
    """Tulis satu titik data ke InfluxDB v2 dengan field 'value' (°C)."""
    if not (INFLUX_URL and INFLUX_TOKEN and INFLUX_ORG and INFLUX_BUCKET):
        raise RuntimeError("Konfigurasi Influx v2 belum lengkap (URL/TOKEN/ORG/BUCKET).")

    def esc_tag(v: str) -> str:
        return str(v).replace(",", r"\,").replace(" ", r"\ ").replace("=", r"\=")

    tags_part = ""
    if tags:
        tags_part = ",".join(f"{k}={esc_tag(v)}" for k, v in tags.items())
        if tags_part:
            tags_part = "," + tags_part

    ts = int(time.time())  # epoch UTC (detik)
    line = f"{measurement}{tags_part} value={field_value_c} {ts}"

    base = INFLUX_URL.rstrip("/") + "/api/v2/write"
    qs = urllib.parse.urlencode({"org": INFLUX_ORG, "bucket": INFLUX_BUCKET, "precision": "s"})
    url = f"{base}?{qs}"

    req = urllib.request.Request(
        url,
        data=line.encode("utf-8"),
        headers={
            "Authorization": f"Token {INFLUX_TOKEN}",
            "Content-Type": "text/plain; charset=utf-8",
            "Accept": "application/json",
        },
        method="POST",
    )
    ctx = _ssl_context_for(url)
    try:
        with urllib.request.urlopen(req, context=ctx, timeout=15) as resp:
            if resp.status not in (200, 204):
                body = resp.read().decode("utf-8", "ignore")
                raise RuntimeError(f"Influx write gagal: HTTP {resp.status} {body}")
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", "ignore")
        raise RuntimeError(f"Influx write gagal: HTTP {e.code} {body}") from None
    except urllib.error.URLError as e:
        raise RuntimeError(f"Influx koneksi gagal: {e}") from None

# ====== WIB minute boundary ======
def _wait_until_next_wib_minute() -> datetime:
    """
    Tidur sampai pergantian menit berikutnya di WIB, lalu kembalikan
    datetime lokal (WIB) target yang detiknya pasti 00.
    """
    now_local = datetime.now(tz=WIB)
    target = now_local.replace(second=0, microsecond=0) + timedelta(minutes=1)
    sleep_s = (target - now_local).total_seconds()
    if sleep_s > 0:
        time.sleep(sleep_s)
    return target

# ====== MAIN LOOP (KIRIM SETIAP MENIT, SINKRON WIB) ======
def main(path=DEFAULT_PATH):
    print(f"Waktu lokal: GMT+{TZ_OFFSET_HOURS} (WIB). Kirim tepat tiap pergantian menit (:00).")
    print(f"File sumber: {path}")

    # Sinkron awal ke menit berikutnya
    _wait_until_next_wib_minute()

    while True:
        tick_local = _wait_until_next_wib_minute()  # detik = 00 (WIB)
        tick_str = tick_local.strftime("%Y-%m-%d %H:%M:%S")

        try:
            xml_bytes = _load_xml_bytes(path)
            tC = read_water_in_temp_c(xml_bytes)
            if tC is None:
                print(f"[{tick_str}] [WARN] Tidak menemukan water temperature (Mixture) untuk 'water in'.")
            else:
                write_to_influx_celsius(
                    measurement="water_temperature_c",
                    field_value_c=tC,
                    tags={"stream": "water_temp"}
                )
                print(f"[{tick_str}] [OK] Terkirim: water temperature = {tC:.3f} °C")
        except Exception as e:
            print(f"[{tick_str}] [ERR] {e}")

if __name__ == "__main__":
    import sys
    path = os.getenv("DWSIM_PATH", DEFAULT_PATH)
    if len(sys.argv) > 1:
        path = sys.argv[1]
    main(path)

