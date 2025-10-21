// main.rs

extern crate alloc;

use anyhow::{bail, Context, Result};
use log::{error, info, warn};

use esp_idf_sys as sys; // C-API (MQTT, HTTP, SNTP, time)

use alloc::ffi::CString;
use alloc::string::String;
use alloc::string::ToString;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    log::EspLogger,
    nvs::EspDefaultNvsPartition,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration as StaCfg, Configuration as WifiCfg, EspWifi},
};

use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{AnyIOPin, Output, PinDriver},
    ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution},
    peripherals::Peripherals,
    uart::{config::Config as UartConfig, UartDriver},
    units::Hertz,
};

use serde_json::json;

// ===== tambahan untuk TCP server & concurrency =====
use std::io::Write as IoWrite;
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

// =================== KONFIGURASI ===================
// Wi-Fi
const WIFI_SSID: &str = "KOST GK 31";
const WIFI_PASS: &str = "12345678";

// ThingsBoard Cloud (MQTT Basic)
const TB_MQTT_URL: &str = "mqtt://mqtt.thingsboard.cloud:1883";
const TB_CLIENT_ID: &str = "esp32s3-eggdhis";
const TB_USERNAME: &str = "esp32s3SKT13";
const TB_PASSWORD: &str = "453621";

// InfluxDB (Cloud atau lokal)
const INFLUX_URL: &str = "https://us-east-1-1.aws.cloud2.influxdata.com";
const INFLUX_ORG_ID: &str = "882113367e216236";
const INFLUX_BUCKET: &str = "skt13eggdhis";
const INFLUX_TOKEN: &str = "w16KRRwJKin17Pn94QudXx8yjhCBkxc-0ZIoRp5zGmXrGOoYyRb2d1j7Lhqyw7-e9hyUtj5WshTk0iIyp8DnPQ==";

// Modbus (SHT20 via RS485)
const MODBUS_ID: u8 = 0x01;
const BAUD: u32 = 9_600;

// TCP Server (untuk stream JSON ke klien)
const TCP_LISTEN_ADDR: &str = "0.0.0.0";
const TCP_LISTEN_PORT: u16 = 7878;

// Relay (ON jika RH > 52%)
const RELAY_ACTIVE_LOW: bool = true;      // true: active-LOW; false: active-HIGH
const RH_ON_THRESHOLD: f32 = 52.0;        // Relay ON bila RH > 52.0

// Interval pembacaan & kirim (ms)
const READ_PERIOD_MS: u64 = 60_000;       // 1 menit

// =================== Util ===================
#[inline(always)]
fn ms_to_ticks(ms: u32) -> u32 {
    (ms as u64 * sys::configTICK_RATE_HZ as u64 / 1000) as u32
}

fn looks_like_uuid(s: &str) -> bool {
    s.len() == 36 && s.matches('-').count() == 4
}

// Minimal percent-encoding untuk komponen query (RFC 3986 unreserved: ALNUM -_.~)
fn url_encode_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.as_bytes() {
        let c = *b as char;
        if c.is_ascii_alphanumeric() || "-_.~".contains(c) {
            out.push(c);
        } else {
            let _ = core::fmt::write(&mut out, format_args!("%{:02X}", b));
        }
    }
    out
}

// =================== RTC / SNTP (fixed for esp-idf-sys) ===================

fn init_rtc_and_wait() {
    unsafe {
        // Set zona waktu: WIB (UTC+7) → POSIX TZ "WIB-7"
        let k_tz = CString::new("TZ").unwrap();
        let v_tz = CString::new("WIB-7").unwrap();
        sys::setenv(k_tz.as_ptr(), v_tz.as_ptr(), 1);
        sys::tzset();

        // SNTP API dengan prefix esp_sntp_ (ESP-IDF v5+)
        sys::esp_sntp_setoperatingmode(sys::esp_sntp_operatingmode_t_ESP_SNTP_OPMODE_POLL);
        // Perhatikan: binding expects *const u8 (c_char)
        sys::esp_sntp_setservername(0, b"id.pool.ntp.org\0".as_ptr());
        sys::esp_sntp_setservername(1, b"time.cloudflare.com\0".as_ptr());
        sys::esp_sntp_setservername(2, b"pool.ntp.org\0".as_ptr());

        sys::esp_sntp_init();

        // Tunggu waktu valid (epoch ≥ ~2017)
        let mut tries = 0;
        loop {
            let mut tv: sys::timeval = core::mem::zeroed();
            sys::gettimeofday(&mut tv as *mut _, core::ptr::null_mut());
            if tv.tv_sec >= 1_500_000_000 {
                break;
            }
            tries += 1;
            if tries > 100 { // ~10 detik
                warn!("SNTP: waktu belum valid, lanjut (akan update ketika siap)");
                break;
            }
            FreeRtos::delay_ms(100);
        }

        // Log waktu lokal (gunakan buffer u8 untuk strftime)
        let mut now: sys::time_t = 0;
        sys::time(&mut now as *mut _);
        let mut tm: sys::tm = core::mem::zeroed();
        sys::localtime_r(&now, &mut tm);
        let mut buf = [0u8; 40];
        let fmt = b"%Y-%m-%d %H:%M:%S %Z\0";
        let n = sys::strftime(buf.as_mut_ptr(), buf.len(), fmt.as_ptr(), &tm) as usize;
        if n > 0 && n < buf.len() {
            let s = core::str::from_utf8(&buf[..n]).unwrap_or("<??>");
            info!("RTC set: {}", s);
        } else {
            info!("RTC set (strftime failed to format)");
        }
    }
}

/// Ambil epoch ms dari RTC (None jika belum valid).
fn rtc_epoch_ms() -> Option<u64> {
    unsafe {
        let mut tv: sys::timeval = core::mem::zeroed();
        sys::gettimeofday(&mut tv as *mut _, core::ptr::null_mut());
        if tv.tv_sec >= 1_500_000_000 {
            Some((tv.tv_sec as u64) * 1000 + (tv.tv_usec as u64) / 1000)
        } else {
            None
        }
    }
}

/// ISO-8601 lokal (WIB): "YYYY-MM-DDTHH:MM:SS"
fn rtc_iso8601() -> Option<String> {
    unsafe {
        let mut now: sys::time_t = 0;
        sys::time(&mut now as *mut _);
        if now < 1_500_000_000 {
            return None;
        }
        let mut tm: sys::tm = core::mem::zeroed();
        sys::localtime_r(&now, &mut tm);
        let mut buf = [0u8; 32];
        let fmt = b"%Y-%m-%dT%H:%M:%S\0";
        let n = sys::strftime(buf.as_mut_ptr(), buf.len(), fmt.as_ptr(), &tm) as usize;
        if n > 0 && n < buf.len() {
            Some(core::str::from_utf8(&buf[..n]).unwrap_or("").to_string())
        } else {
            None
        }
    }
}

// =================== MQTT client (C-API) ===================
struct SimpleMqttClient {
    client: *mut sys::esp_mqtt_client,
}

impl SimpleMqttClient {
    fn new(broker_url: &str, username: &str, password: &str, client_id: &str) -> Result<Self> {
        unsafe {
            let broker_url_cstr = CString::new(broker_url)?;
            let username_cstr = CString::new(username)?;
            let password_cstr = CString::new(password)?;
            let client_id_cstr = CString::new(client_id)?;

            let mut cfg: sys::esp_mqtt_client_config_t = core::mem::zeroed();
            cfg.broker.address.uri = broker_url_cstr.as_ptr() as *const u8;
            cfg.credentials.username = username_cstr.as_ptr() as *const u8;
            cfg.credentials.client_id = client_id_cstr.as_ptr() as *const u8;
            cfg.credentials.authentication.password = password_cstr.as_ptr() as *const u8;
            cfg.session.keepalive = 30; // detik
            cfg.network.timeout_ms = 20_000; // ms

            let client = sys::esp_mqtt_client_init(&cfg);
            if client.is_null() {
                bail!("Failed to initialize MQTT client");
            }
            let err = sys::esp_mqtt_client_start(client);
            if err != sys::ESP_OK {
                bail!("Failed to start MQTT client, esp_err=0x{:X}", err as u32);
            }

            sys::vTaskDelay(ms_to_ticks(2500));
            Ok(Self { client })
        }
    }

    fn publish(&self, topic: &str, data: &str) -> Result<()> {
        unsafe {
            let topic_c = CString::new(topic)?;
            let msg_id = sys::esp_mqtt_client_publish(
                self.client,
                topic_c.as_ptr(),
                data.as_ptr(),
                data.len() as i32,
                1,
                0,
            );
            if msg_id < 0 {
                bail!("Failed to publish message, code: {}", msg_id);
            }
            info!("MQTT published (id={})", msg_id);
            Ok(())
        }
    }
}

impl Drop for SimpleMqttClient {
    fn drop(&mut self) {
        unsafe {
            sys::esp_mqtt_client_stop(self.client);
            sys::esp_mqtt_client_destroy(self.client);
        }
    }
}

// =================== CRC & Modbus util ===================
fn crc16_modbus(mut crc: u16, byte: u8) -> u16 {
    crc ^= byte as u16;
    for _ in 0..8 {
        crc = if (crc & 1) != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
    }
    crc
}
fn modbus_crc(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data { crc = crc16_modbus(crc, b); }
    crc
}
fn build_read_req(slave: u8, func: u8, start_reg: u16, qty: u16) -> heapless::Vec<u8, 256> {
    use heapless::Vec;
    let mut pdu: Vec<u8, 256> = Vec::new();
    pdu.push(slave).unwrap();
    pdu.push(func).unwrap();
    pdu.push((start_reg >> 8) as u8).unwrap();
    pdu.push((start_reg & 0xFF) as u8).unwrap();
    pdu.push((qty >> 8) as u8).unwrap();
    pdu.push((qty & 0xFF) as u8).unwrap();
    let crc = modbus_crc(&pdu);
    pdu.push((crc & 0xFF) as u8).unwrap();
    pdu.push((crc >> 8) as u8).unwrap();
    pdu
}
fn parse_read_resp(expected_slave: u8, qty: u16, buf: &[u8]) -> Result<heapless::Vec<u16, 64>> {
    use heapless::Vec;
    if buf.len() >= 5 && (buf[1] & 0x80) != 0 {
        let crc_rx = u16::from(buf[4]) << 8 | u16::from(buf[3]);
        let crc_calc = modbus_crc(&buf[..3]);
        if crc_rx == crc_calc {
            let code = buf[2];
            bail!("Modbus exception 0x{:02X}", code);
        } else {
            bail!("Exception frame CRC mismatch");
        }
    }
    let need = 1 + 1 + 1 + (2 * qty as usize) + 2;
    if buf.len() < need { bail!("Response too short: got {}, need {}", buf.len(), need); }
    if buf[0] != expected_slave { bail!("Unexpected slave id: got {}, expected {}", buf[0], expected_slave); }
    if buf[1] != 0x03 && buf[1] != 0x04 { bail!("Unexpected function code: 0x{:02X}", buf[1]); }
    let bc = buf[2] as usize;
    if bc != 2 * qty as usize { bail!("Unexpected byte count: {}", bc); }
    let crc_rx = u16::from(buf[need - 1]) << 8 | u16::from(buf[need - 2]);
    let crc_calc = modbus_crc(&buf[..need - 2]);
    if crc_rx != crc_calc { bail!("CRC mismatch: rx=0x{:04X}, calc=0x{:04X}", crc_rx, crc_calc); }

    let mut out: Vec<u16, 64> = Vec::new();
    for i in 0..qty as usize {
        let hi = buf[3 + 2 * i] as u16;
        let lo = buf[3 + 2 * i + 1] as u16;
        out.push((hi << 8) | lo).unwrap();
    }
    Ok(out)
}

// =================== RS485 helpers ===================
fn rs485_write(
    uart: &UartDriver<'_>,
    de: &mut PinDriver<'_, esp_idf_svc::hal::gpio::Gpio21, Output>,
    data: &[u8],
) -> Result<()> {
    de.set_high()?;
    FreeRtos::delay_ms(3);
    uart.write(data)?;
    uart.wait_tx_done(200)?;
    de.set_low()?;
    FreeRtos::delay_ms(3);
    Ok(())
}
fn rs485_read(uart: &UartDriver<'_>, dst: &mut [u8], ticks: u32) -> Result<usize> {
    uart.clear_rx()?;
    let n = uart.read(dst, ticks)?;
    use core::fmt::Write as _;
    let mut s = String::new();
    for b in &dst[..n] { write!(&mut s, "{:02X} ", b).ok(); }
    info!("RS485 RX {} bytes: {}", n, s);
    Ok(n)
}
fn try_read(
    uart: &UartDriver<'_>,
    de: &mut PinDriver<'_, esp_idf_svc::hal::gpio::Gpio21, Output>,
    func: u8, start: u16, qty: u16, ticks: u32,
) -> Result<heapless::Vec<u16, 64>> {
    let req = build_read_req(MODBUS_ID, func, start, qty);
    rs485_write(uart, de, &req)?;
    let mut buf = [0u8; 64];
    let n = rs485_read(uart, &mut buf, ticks)?;
    parse_read_resp(MODBUS_ID, qty, &buf[..n])
}
fn probe_map(
    uart: &UartDriver<'_>,
    de: &mut PinDriver<'_, esp_idf_svc::hal::gpio::Gpio21, Output>,
) -> Option<(u8, u16, u16)> {
    for &fc in &[0x04u8, 0x03u8] {
        for start in 0x0000u16..=0x0010u16 {
            for &qty in &[1u16, 2u16] {
                if let Ok(regs) = try_read(uart, de, fc, start, qty, 250) {
                    info!(
                        "FOUND: fc=0x{:02X}, start=0x{:04X}, qty={}, regs={:04X?}",
                        fc, start, qty, regs.as_slice()
                    );
                    return Some((fc, start, qty));
                }
            }
        }
    }
    None
}
fn read_sht20_with_map(
    uart: &UartDriver<'_>,
    de: &mut PinDriver<'_, esp_idf_svc::hal::gpio::Gpio21, Output>,
    fc: u8, start: u16, qty: u16,
) -> Result<(f32, f32)> {
    let regs = try_read(uart, de, fc, start, qty, 250)?;
    let (raw_t, raw_h) = if regs.len() >= 2 { (regs[0], regs[1]) } else { (regs[0], 0) };
    let temp_c = (raw_t as f32) * 0.1;
    let rh_pct = (raw_h as f32) * 0.1;
    Ok((temp_c, rh_pct))
}

// =================== Wi-Fi (BlockingWifi) ===================
fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> Result<()> {
    let cfg = WifiCfg::Client(StaCfg {
        ssid: heapless::String::try_from(WIFI_SSID).unwrap(),
        password: heapless::String::try_from(WIFI_PASS).unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        channel: None,
        ..Default::default()
    });
    wifi.set_configuration(&cfg)?;
    wifi.start()?;
    info!("Wi-Fi driver started");
    wifi.connect()?;
    info!("Wi-Fi connect issued, waiting for netif up ...");
    wifi.wait_netif_up()?;
    let ip = wifi.wifi().sta_netif().get_ip_info()?;
    info!("Wi-Fi connected. IP = {}", ip.ip);
    unsafe { sys::vTaskDelay(ms_to_ticks(1200)); }
    Ok(())
}

// =================== Influx helpers ===================

fn build_influx_lp(measurement: &str, device: &str, t_c: f32, h_pct: f32, ts_ms: Option<u64>) -> String {
    // Line Protocol: <measurement>,tagK=tagV fieldK1=v1,fieldK2=v2 <timestamp_ms>
    // tag: device
    let base = format!("{},device={} temperature_c={},humidity_pct={}",
                       measurement, device, t_c, h_pct);
    if let Some(ms) = ts_ms {
        format!("{} {}", base, ms)
    } else {
        base
    }
}

fn influx_write(lp: &str) -> Result<()> {
    unsafe {
        let org_q = if looks_like_uuid(INFLUX_ORG_ID) { "orgID" } else { "org" };
        let url = format!(
            "{}/api/v2/write?{}={}&bucket={}&precision=ms",
            INFLUX_URL,
            org_q,
            url_encode_component(INFLUX_ORG_ID),
            url_encode_component(INFLUX_BUCKET)
        );
        let url_c = CString::new(url.as_str())?;

        let mut cfg: sys::esp_http_client_config_t = core::mem::zeroed();
        cfg.url = url_c.as_ptr();
        cfg.method = sys::esp_http_client_method_t_HTTP_METHOD_POST;

        if INFLUX_URL.starts_with("https://") {
            cfg.transport_type = sys::esp_http_client_transport_t_HTTP_TRANSPORT_OVER_SSL;
            cfg.crt_bundle_attach = Some(sys::esp_crt_bundle_attach);
        }

        let client = sys::esp_http_client_init(&cfg);
        if client.is_null() {
            bail!("esp_http_client_init failed");
        }

        // headers
        let h_auth = CString::new("Authorization")?;
        let v_auth = CString::new(format!("Token {}", INFLUX_TOKEN))?;
        let h_ct = CString::new("Content-Type")?;
        let v_ct = CString::new("text/plain; charset=utf-8")?;
        let h_acc = CString::new("Accept")?;
        let v_acc = CString::new("application/json")?;
        let h_conn = CString::new("Connection")?;
        let v_conn = CString::new("close")?;

        sys::esp_http_client_set_header(client, h_auth.as_ptr(), v_auth.as_ptr());
        sys::esp_http_client_set_header(client, h_ct.as_ptr(), v_ct.as_ptr());
        sys::esp_http_client_set_header(client, h_acc.as_ptr(), v_acc.as_ptr());
        sys::esp_http_client_set_header(client, h_conn.as_ptr(), v_conn.as_ptr());

        // body (Line Protocol)
        sys::esp_http_client_set_post_field(client, lp.as_ptr(), lp.len() as i32);

        // perform
        let err = sys::esp_http_client_perform(client);
        if err != sys::ESP_OK {
            let e = format!("esp_http_client_perform failed: 0x{:X}", err as u32);
            sys::esp_http_client_cleanup(client);
            bail!(e);
        }

        let status = sys::esp_http_client_get_status_code(client);
        if status != 204 {
            let mut body_buf = [0u8; 256];
            let read = sys::esp_http_client_read_response(client, body_buf.as_mut_ptr(), body_buf.len() as i32);
            let body = if read > 0 {
                core::str::from_utf8(&body_buf[..read as usize]).unwrap_or("")
            } else { "" };
            warn!("Influx write failed: HTTP {} Body: {}", status, body);
            sys::esp_http_client_cleanup(client);
            bail!("Influx write HTTP status {}", status);
        } else {
            info!("✅ Data Berhasil Dikirim ke InfluxDB");
        }
        sys::esp_http_client_cleanup(client);
        Ok(())
    }
}

// =================== Servo helpers (LEDC) ===================
struct Servo {
    ch: LedcDriver<'static>,
    duty_0: u32,
    duty_90: u32,
    duty_180: u32,
}

impl Servo {
    fn new(mut ch: LedcDriver<'static>) -> Result<Self> {
        let max = ch.get_max_duty() as u64; // Bits14 → 16383
        let period_us = 20_000u64;          // 50 Hz → 20 ms
        let duty_from_us = |us: u32| -> u32 { ((max * us as u64) / period_us) as u32 };

        // Sesuaikan jika perlu: 1000–2000 us untuk beberapa servo.
        let duty_0 = duty_from_us(500);    // ~0.5 ms (≈0°)
        let duty_90 = duty_from_us(1500);  // ~1.5 ms (≈90°)
        let duty_180 = duty_from_us(2500); // ~2.5 ms (≈180°)

        ch.set_duty(duty_90)?;
        ch.enable()?;

        Ok(Self { ch, duty_0, duty_90, duty_180 })
    }

    fn set_0(&mut self) -> Result<()> { self.ch.set_duty(self.duty_0).map_err(Into::into) }
    fn set_90(&mut self) -> Result<()> { self.ch.set_duty(self.duty_90).map_err(Into::into) }
    fn set_180(&mut self) -> Result<()> { self.ch.set_duty(self.duty_180).map_err(Into::into) }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum ServoPos { P0, P90, P180 }

// =================== TCP server helpers ===================

fn start_tcp_server() -> mpsc::Sender<String> {
    let (tx, rx) = mpsc::channel::<String>();
    let clients: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));

    // Thread acceptor
    {
        let clients_accept = Arc::clone(&clients);
        thread::spawn(move || {
            let addr = format!("{}:{}", TCP_LISTEN_ADDR, TCP_LISTEN_PORT);
            loop {
                match TcpListener::bind(&addr) {
                    Ok(listener) => {
                        info!("TCP Server listening on {}", addr);
                        listener.set_nonblocking(true).ok(); // non-blocking accept
                        loop {
                            match listener.accept() {
                                Ok((stream, peer)) => {
                                    let _ = stream.set_nodelay(true);
                                    info!("TCP client connected: {}", peer);
                                    if let Ok(mut vec) = clients_accept.lock() {
                                        vec.push(stream);
                                    }
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    FreeRtos::delay_ms(100);
                                }
                                Err(e) => {
                                    warn!("TCP accept error: {} (rebind)", e);
                                    FreeRtos::delay_ms(1000);
                                    break;
                                }
                            }
                            FreeRtos::delay_ms(10);
                        }
                    }
                    Err(e) => {
                        warn!("TCP bind {} error: {} (retry in 1s)", addr, e);
                        FreeRtos::delay_ms(1000);
                    }
                }
            }
        });
    }

    // Thread broadcaster (writer)
    {
        let clients_write = Arc::clone(&clients);
        thread::spawn(move || {
            while let Ok(line) = rx.recv() {
                if let Ok(mut vec) = clients_write.lock() {
                    vec.retain_mut(|stream| {
                        if writeln!(stream, "{}", line).is_err() {
                            warn!("TCP write to client failed: drop client");
                            false
                        } else {
                            true
                        }
                    });
                }
            }
        });
    }

    tx
}

// ===== Relay helper =====
#[inline(always)]
fn set_relay(
    relay: &mut PinDriver<'_, esp_idf_svc::hal::gpio::Gpio5, Output>,
    on: bool,
    active_low: bool,
) -> anyhow::Result<()> {
    if active_low {
        if on { relay.set_low()?; } else { relay.set_high()?; }
    } else {
        if on { relay.set_high()?; } else { relay.set_low()?; }
    }
    Ok(())
}

// Helper: satu siklus baca + publish + tulis Influx + kirim ke TCP server
fn do_sensor_io(
    uart: &UartDriver<'_>,
    de_pin: &mut PinDriver<'_, esp_idf_svc::hal::gpio::Gpio21, Output>,
    fc_use: u8, start_use: u16, qty_use: u16,
    mqtt: &SimpleMqttClient,
    topic_tele: &str,
    tcp_tx: &mpsc::Sender<String>,
) -> Result<(f32, f32)> {
    let (t, h) = read_sht20_with_map(uart, de_pin, fc_use, start_use, qty_use)?;

    // Ambil waktu RTC (fallback ke esp_timer bila belum valid)
    let ts_ms_rtc = rtc_epoch_ms();
    let ts_ms = ts_ms_rtc.unwrap_or_else(|| unsafe { (sys::esp_timer_get_time() as u64) / 1000 });

    let t_rounded = (t * 10.0).round() / 10.0;
    let h_rounded = (h * 10.0).round() / 10.0;

    let payload = json!({
        "esp32s3 Temperature": t_rounded,
        "esp32s3 Humidity": h_rounded,
        "rtc_ms": ts_ms_rtc,        // None → null jika RTC belum valid
        "rtc_iso": rtc_iso8601(),   // None → null jika RTC belum valid
        "ts_ms": ts_ms              // selalu ada (RTC atau esp_timer)
    }).to_string();

    // 1) log ke stdout
    println!("{}", payload);

    // 2) publish ke ThingsBoard via MQTT
    if let Err(e) = mqtt.publish(topic_tele, &payload) {
        error!("MQTT publish error: {e:?}");
    }

    // 3) tulis ke Influx (HTTP) pakai timestamp RTC bila ada
    let lp = build_influx_lp("sht20", TB_CLIENT_ID, t_rounded, h_rounded, ts_ms_rtc);
    if let Err(e) = influx_write(&lp) {
        warn!("Influx write failed: {e}");
    }

    // 4) kirim ke semua klien TCP yang terhubung
    if let Err(e) = tcp_tx.send(payload) {
        warn!("TCP channel send failed: {e}");
    }

    Ok((t, h))
}

// =================== main ===================
fn main() -> Result<()> {
    // ESP-IDF init
    sys::link_patches();
    EspLogger::initialize_default();
    info!("🚀 Modbus RS485 + ThingsBoard MQTT + InfluxDB + Servo + TCP Server + Relay + RTC (SNTP) | 1 menit interval");

    // Peripherals & services
    let peripherals = Peripherals::take().context("Peripherals::take")?;
    let pins = peripherals.pins;
    let sys_loop = EspSystemEventLoop::take().context("eventloop")?;
    let nvs = EspDefaultNvsPartition::take().context("nvs")?;

    // Wi-Fi via BlockingWifi
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;
    connect_wifi(&mut wifi)?;

    // *** RTC: sync waktu via SNTP, set TZ ke WIB, tunggu valid ***
    init_rtc_and_wait();

    // MQTT ThingsBoard (Basic)
    let mqtt = SimpleMqttClient::new(TB_MQTT_URL, TB_USERNAME, TB_PASSWORD, TB_CLIENT_ID)?;
    info!("MQTT connected to {}", TB_MQTT_URL);

    // === Start TCP Server (listener) ===
    let tcp_tx = start_tcp_server();
    info!("TCP server spawned at {}:{}", TCP_LISTEN_ADDR, TCP_LISTEN_PORT);

    // UART0 + RS485 (GPIO43/44, DE: GPIO21)
    let tx = pins.gpio43; // U0TXD
    let rx = pins.gpio44; // U0RXD
    let de = pins.gpio21;
    let cfg = UartConfig::new().baudrate(Hertz(BAUD));
    let uart = UartDriver::new(peripherals.uart0, tx, rx, None::<AnyIOPin>, None::<AnyIOPin>, &cfg)
        .context("UartDriver::new")?;
    let mut de_pin = PinDriver::output(de).context("PinDriver::output(DE)")?;
    de_pin.set_low()?; // default RX
    info!("UART0 ready (TX=GPIO43, RX=GPIO44, DE=GPIO21), {} bps", BAUD);

    // ===== Servo init (LEDC 50 Hz, 14-bit) =====
    let ledc = peripherals.ledc;
    let mut servo_timer = LedcTimerDriver::new(
        ledc.timer0,
        &TimerConfig {
            frequency: Hertz(50),
            resolution: Resolution::Bits14,
            ..Default::default()
        },
    )?;
    let servo_channel = LedcDriver::new(ledc.channel0, &mut servo_timer, pins.gpio18)?;
    let mut servo = Servo::new(servo_channel)?;
    let mut servo_pos = ServoPos::P90; // posisi awal 90°

    // === Relay init (GPIO5) ===
    let mut relay = PinDriver::output(pins.gpio5).context("PinDriver::output(RELAY GPIO5)")?;
    if RELAY_ACTIVE_LOW { relay.set_high()?; } else { relay.set_low()?; }
    info!("Relay siap di GPIO5 (active-{}).", if RELAY_ACTIVE_LOW { "LOW" } else { "HIGH" });

    // Probe mapping registri SHT20 (opsional)
    let (mut fc_use, mut start_use, mut qty_use) = (0x04u8, 0x0000u16, 2u16);
    if let Some((fc, start, qty)) = probe_map(&uart, &mut de_pin) {
        (fc_use, start_use, qty_use) = (fc, start, qty);
        info!("Using map: fc=0x{:02X}, start=0x{:04X}, qty={}", fc_use, start_use, qty_use);
    } else {
        warn!("Probe failed. Fallback map: fc=0x{:02X}, start=0x{:04X}, qty={}", fc_use, start_use, qty_use);
    }

    // ====== Penjadwalan 1 menit ======
    // Tentukan waktu bacaan pertama: align ke menit berikutnya jika RTC valid, kalau tidak mulai 60s dari sekarang.
    let now_ms_boot: u64 = unsafe { (sys::esp_timer_get_time() as u64) / 1000 };
    let mut next_read_ms: u64 = if let Some(rtc_ms) = rtc_epoch_ms() {
        // align ke boundary menit berikutnya
        rtc_ms - (rtc_ms % READ_PERIOD_MS) + READ_PERIOD_MS
    } else {
        now_ms_boot + READ_PERIOD_MS
    };

    let topic_tele = "v1/devices/me/telemetry";

    loop {
        // Waktu "wall clock" kalau tersedia; fallback ke esp_timer jika belum valid
        let now_ms: u64 = rtc_epoch_ms().unwrap_or_else(|| unsafe { (sys::esp_timer_get_time() as u64) / 1000 });

        if now_ms + 5 >= next_read_ms && now_ms >= next_read_ms {
            // Saatnya membaca + kirim (tiap 1 menit)
            match do_sensor_io(&uart, &mut de_pin, fc_use, start_use, qty_use, &mqtt, topic_tele, &tcp_tx) {
                Ok((t, h)) => {
                    // === Relay logic: ON bila RH > threshold ===
                    let want_on = h > RH_ON_THRESHOLD;
                    set_relay(&mut relay, want_on, RELAY_ACTIVE_LOW)?;
                    let lvl = relay.is_set_high();
                    info!(
                        "Relay {} (RH={:.1}%) | GPIO5 level={}",
                        if want_on { "ON" } else { "OFF" },
                        h,
                        if lvl { "HIGH" } else { "LOW" }
                    );

                    // Aturan servo sederhana berdasar suhu (opsional)
                    if t < 33.5 {
                        if servo_pos != ServoPos::P180 {
                            if let Err(e) = servo.set_180() { error!("Servo set 180° error: {e:?}"); }
                            else { info!("Servo → 180° (T={:.1}°C)", t); servo_pos = ServoPos::P180; }
                        }
                    } else if t > 33.5 {
                        if servo_pos != ServoPos::P0 {
                            if let Err(e) = servo.set_0() { error!("Servo set 0° error: {e:?}"); }
                            else { info!("Servo → 0° (T={:.1}°C)", t); servo_pos = ServoPos::P0; }
                        }
                    } else {
                        info!("T=33.5°C persis → Servo tetap di {:?}", servo_pos);
                    }
                }
                Err(e) => error!("Modbus read error: {e:?}"),
            }

            // Jadwalkan menit berikutnya
            next_read_ms = next_read_ms.saturating_add(READ_PERIOD_MS);

            // Jika ada lompatan waktu (mis. RTC baru sync) dan next_read_ms sudah di masa lalu jauh,
            // reset jadwal ke 1 menit dari sekarang.
            if next_read_ms + (5 * READ_PERIOD_MS) < now_ms || next_read_ms < now_ms {
                next_read_ms = now_ms + READ_PERIOD_MS;
            }
        } else {
            // Belum waktunya → tidur pendek untuk hemat CPU
            FreeRtos::delay_ms(250);
        }
    }
}

