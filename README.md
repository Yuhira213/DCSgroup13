# “SISTEM KONTROL TERDISTRIBUSI - Sistem Monitoring dan Kontrol Terintegrasi”
Proyek ini dikembangkan untuk memenuhi tugas Sistem Komputasi Terdistribusi (SKT) yang menggabungkan pembacaan sensor SHT20, simulasi DWSIM, database InfluxDB, dan platform IoT ThingsBoard.

 ## Authors
1. Ahmad Radhy                  (Supervisor)
2. Yudhistira Ananda Kuswantoro (2042231015)
3. Egga Terbyd Fabryan          (2042231029)

# Untuk Laporan dapat diakses di tautan berikut:
## Link Laporan:
### https://its.id/m/LaporanSKTkel13

## Struktur Proyek

├── DWsim/
│   ├── dwsim.py          # Python bridge → InfluxDB
│   └── Thingsboard.py    # Python bridge from InfluxDB → Thingsboard
└── Modbus-Edge/
    ├── src/              # Source code utama ESP32-S3
    ├── Cargo.toml        # Konfigurasi & dependensi proyek Rust
    ├── build.rs          # Script build firmware
    ├── target/           # Hasil build firmware
    └── influxdb.py       # Code untuk mengirim data bacaan sensor ke InfluxDB


## Requirements
### Hardware
1. ESP32S3
2. Industrial SHT40 Temperature and Humidity Sensor
3. TTL to RS485 MAX485
4. Mini Servo SG90
5. Relay Module 5V High Level Trigger
   
### Software
1. Rust Programming Language V 1.77
2. ESP IDF V 5.2.5
3. esp-idf-svc "=0.51.0"
4. esp-idf-sys = { version = "=0.36.1", features = ["binstart"] }
5. extensa-esp32s3-espidf
6. ldproxy
7. Python 3.13
8. InfluxDB Client
9. Thingsboard

## Koneksi Hardware
| Komponen | Pin ESP32-S3 | Keterangan |
|-----------|---------------|-------------|
| Servo | GPIO18 | Sinyal PWM (LEDC 50 Hz) |
| Relay IN | GPIO5 | Relay Menyalakan Kipas |
| TTL to RS485 MAX485 RO| RX | Komunikasi RS485 |
| TTL to RS485 MAX485 DI| TX | Komunikasi RS485 |
| TTL to RS485 MAX485 RE & DE| GPIO21 | Komunikasi RS485 |

# Langkah Menjalankan Program
## Diharapkan untuk mengunduh zip atau melakukan clone semua isi repository ini dengan memasukkan
```
https://github.com/Yuhira213/DCSgroup13.git
```
## Terlebih dahulu

## Menjalankan Program Rust ke ESP32S3
### 1. Ubah Konfigurasi pada Program di /Modbus-Edge/src/main.rs
<img width="938" height="476" alt="Screenshot from 2025-10-21 10-29-37" src="https://github.com/user-attachments/assets/56b3d72f-ac00-4176-8387-358067bbc6d5" />
## Sesuaikan Wi-Fi, Thingsboard Cloud/Demo, dan InfluxDB dengan yang dimiliki atau digunakan, kemudian save program

### 2. Hubungkan ESP32S3 dengan Komputer
### 3. Klik kanan pada folder kemudian klik ‘Open in Terminal’
kemudian masukkan command cd Modbus-Edge kemudian enter, lalu masukkan command 
```
cargo build --release
```
### Kemudian
```
cargo run --release
```
### Ini digunakan untuk membuild Program Rust, dan menjalankannya di ESP32S3, ketika program telah berjalan. Maka Program akan mengirimkan data bacaan sensor ke Thingsboard, dan InfluxDB Cloud setiap 1 menit

### 4. Lalu buat tab baru untuk terminal,
<img width="878" height="651" alt="Screenshot from 2025-10-20 20-16-18" src="https://github.com/user-attachments/assets/39a2615a-21d8-4274-9a54-ce7b1667ccb3" />
Copy IP dari Wi-Fi yang terhubung dengan ESP32S3

### Ubah Konfigurasi pada file Modbus-Edge/influxdb.py, sesuaikan dengan InfluxDB local yang digunakan
<img width="1077" height="97" alt="image" src="https://github.com/user-attachments/assets/f30cb1e4-c28b-4402-8229-9c2b76f3902f" />
kemudian save file

### Lalu jalankan Command berikut pada terminal
### Contoh:
```
python3 influxdb.py 192.168.1.20 7878
```
### Setelah dijalankan Program menerima bacaan sensor dari ESP32S3 lalu mengirimkannya ke InfluxDB Local

## Untuk mengirimkan Nilai dari DWsim ke InfluxDB dan Thingsboard
### 1. Sesuaikan konfigurasi dari file /DWsim/dwsim.py berikut dengan Influx DB yang digunakan
<img width="1074" height="144" alt="image" src="https://github.com/user-attachments/assets/e4b64096-b375-4ddf-abaf-26f0d6e6a11b" />
### Untuk Path dari Sumber Data, setelah anda membuat plant kemudian menyimpan plant tersebut, ubah path dari datanya disesuaikan dengan lokasi dari save file DWsim.
### Program ini berfungsi untuk membaca nilai temperature yang tersimpan dalam file XML dari save file Plant DWsim, kemudian mengirimkannya ke InfluxDB

### 2. Sesuaikan konfigurasi dari file /DWsim/Thingsboard.py berikut dengan sumber InfluxDB dan Thingsboard yang digunakan
<img width="1101" height="376" alt="Screenshot from 2025-10-21 10-40-15" src="https://github.com/user-attachments/assets/3cf06279-2771-4f9d-a5aa-4087f6d1215d" />
### Program ini berfungsi untuk mengambil data dari InfluxDB Local kemudian mengirimkannya ke Thingsboard.

## Setelah semua program dijalankan, maka nilai bacaan sensor akan terkirim ke InfluxDB Local dan Thingsboard


# Cara Kerja aktuator dalam program
## 1. Kerja Motor Servo
### Posisi awal servo adalah 90°, ketika temperature berada di atas 33.5°C maka servo akan memutar ke 0°, ketika temperatur berada di bawah 33.5° servo akan memutar ke 180°
Sistem kerja ini dapat diubah di bagian // Aturan servo sederhana berdasar suhu

## 2. Kerja Relay
### Relay menggunakan jenis High Level Trigger, Ketika Kelembaban berada di atas 52% maka relay akan menyala, bila kelembaban berada di bawah 52% maka relay akan mati.
Sistem kerja ini dapat diubah di bagian RH_ON_THRESHOLD
 
## Tampilan Program yang berjalan dan Dashboard
### 1. Tampilan Grafik pada InfluxDB
<img width="1543" height="959" alt="image" src="https://github.com/user-attachments/assets/63dd34f6-9c9e-43e8-b44b-bf2ff4055ed6" />
<img width="1602" height="990" alt="image" src="https://github.com/user-attachments/assets/ca3bf36a-1c65-46ac-a5ed-cb0c25f7514b" />

### 2. Tampilan Dashboard Thingsboard
<img width="1803" height="960" alt="image" src="https://github.com/user-attachments/assets/f9045fd1-2770-48e4-8c98-8227a90cde6d" />

 






