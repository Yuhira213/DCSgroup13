# 🍞 “SISTEM PENGENDALIAN EXHAUST FAN UNTUK PENGENDALIAN TEMPERATUR DAN KELEMBABAN KAMAR”
<img width="1803" height="960" alt="image" src="https://github.com/user-attachments/assets/f9045fd1-2770-48e4-8c98-8227a90cde6d" />

Dalam era industri modern, kebutuhan akan sistem pemantauan lingkungan yang andal dan terintegrasi semakin meningkat.

 ## Authors
1. Ahmad Radhy                  (Supervisor)
2. Yudhistira Ananda Kuswantoro (2042231015)
3. Egga Terbyd Fabryan          (2042231029)

# Untuk Laporan dan Powerpoint juga dapat diakses di tautan berikut:
## Link Laporan:
### https://its.id/m/LaporanSKTkel13

## Features
1. **Penyimpanan dan Tampilan Dashboard melalui Thingsboard**
2. **Hardware dapat mengirim data melalui Wi-Fi**
3. **Sistem Database menggunakan InfluDB**

## Requirements
### Software
1. Rust Programming Language V 1.77
2. ESPasdadasdasdasddasdsadasdasdasdasdasdasdasasdasdassadaasdadasdasdasdasdasdadasddddddadwatest
3. esp-idf-svc "=0.51.0"
4. esp-idf-sys = { version = "=0.36.1", features = ["binstart"] }
5. extensa-esp32s3-espidf
6. ldproxy
7. Python 3.13
8. Industrial SHT20 Temperature and Humidity Sensor
9. Modbus To TTL Converter
10. InfluxDB Client
11. 

### Extension
1. Metamask

### Library for PyQT5
1. tkinter
2. io
3. matplotlib
4. collections
5. requests
6. serde
7. reqwest
atau bisa juga dengan memasukkan command berikut:
```
pip3 install matplotlib requests tkinter io serde reqwest collections
```


# Langkah Pengkomunikasian Seluruh Sistem
## Diharapkan untuk mengunduh zip atau melakukan clone semua isi repository ini dengan memasukkan
```
https://github.com/Yuhira213/InterconnectionSystemForStorageMonitoring.git
```
## Terlebih dahulu

### 1. Klik kanan pada folder kemudian klik ‘Open in Terminal’
kemudian masukkan command cd sensor kemudian enter, lalu masukkan command 
```
cargo run
```
lalu enter, untuk membaca kemudian mengirimkan data dari bacaan sensor ke Rust TCP Server. Maka tampilannya akan seperti berikut:
![WhatsApp Image 2025-06-19 at 01 49 57_04ee2861](https://github.com/user-attachments/assets/1a3b32ee-89fe-4a1e-be3f-ec3af137b3ac)
terlihat pada tampilan awal tertera “Gagal baca sensor: Permission denied.

### 2. Lalu buat tab baru untuk terminal, 
kemudian masukkan command berikut
```
sudo dmesg | grep ttyUSB*
```
berguna untuk mengidentifikasi koneksi dari USB konverter dari modbus sensor SHT20
kemudian command
```
sudo chmod a+rw /dev/ttyUSB0
(sesuaikan dengan deteksi yang ditampilkan)
```
untuk mendapatkan permission untuk membaca data dari sensor

![image](https://github.com/user-attachments/assets/05124d78-52aa-4365-b2fd-a28afc1d9c35)

Maka setelah itu tampilan dari Terminal dari sensor akan berubah menjadi berikut:

![image](https://github.com/user-attachments/assets/9f869be4-cba3-4ebb-8f3a-35911d182e3e)

### 3. Lalu buat tab baru untuk terminal,
kemudian masukkan command ```ganache``` , kemudian akan muncul tampilan seperti gambar berikut:

![image](https://github.com/user-attachments/assets/d581a9a3-fae3-463e-9ee6-c331ff982113)

setelah itu salin salah satu Private Keys yang diberikan dari tampilan tersebut

![image](https://github.com/user-attachments/assets/3ec94ad9-442f-49e2-924b-0cbe87a82cb5)

### 4. Kemudian buka file main.rs yang terdapat di /Server/src. Kemudian ganti local wallet yang ada di bawah “let wallet: LocalWallet =” kemudian save progressnyaa.
![image](https://github.com/user-attachments/assets/c4c5fcad-a912-423a-8d28-87b78a290144)

### 5. Ganti Influx Token yanga ada pada File main.rs di Sensor/Src/
kemudian sesuaikan bagian yang disorot dengan Influx Token yang dimiliki

![image](https://github.com/user-attachments/assets/6a907846-3908-4048-8daa-d1a0e7f801bd)

Kemudian sesuaikan dengan mengganti org dan bucket yang ada pada "/write?org=ITS&bucket=ISImonitor&precision=s" dengan yang dimiliki dan dipakai saat ini

### 6. Klik untuk tambahkan tab baru dari Terminal
kemudian masukkan command cd Server kemudian enter, lalu masukkan command
```bash
cargo run
```
lalu enter, untuk menjalankan Rust TCP Server dan mengirimkan data dari bacaan sensor yang dikirim ke TCP Server juga dikirimkan ke InfluxDB. Tampilan terminal akan seperti berikut:

![image](https://github.com/user-attachments/assets/dfb52371-1eed-4b9b-a284-0ffb6e22edb4)

Setelah itu salin Smart contract yang tertera pada hasil tampilan terminal

### 7. Kemudian buka script.js yang ada di /Web3 with features
ganti contract Adress yang ada di dalamnya(yang sudah disorot) dengan Smart contract yang sudah disalin tadi lalu save progressnya.

![image](https://github.com/user-attachments/assets/0336bea5-74c3-43cb-9a5e-482e32fb0a20)

### 8. Kemudian klik kanan pada Folder ‘Web3 with feature’ lalu klik open in terminal, 
kemudian jalankan command 
```
python3 -m http.server 8005
```
kemudian enter, untuk menjalankan halaman Web3 dari program sistem.

![image](https://github.com/user-attachments/assets/6c8e2733-7e30-4a2a-b926-1ed9952ab6f3)

setelah itu klik kanan pada (http://0.0.0.0:8005/) kemudian open link

### 9. Kemudian akan diarahkan ke pada Web3 tampilan berikut.
![image](https://github.com/user-attachments/assets/55d6cff6-f092-4884-9f49-73fdc27d05c0)

### 10. Kemudian klik pada extension di dalam firefox lalu klik pada Metamask untuk menghubungkan blockchain.
![image](https://github.com/user-attachments/assets/a6790be1-3915-41dd-8612-9491e4dc6831)

- Setelah itu klik pada tampilan kiri,
  
  ![image](https://github.com/user-attachments/assets/9d57de3e-00f2-4e0f-8c09-efacb16652d0)
  
- Lalu klik ‘add a custom network’
  
  ![image](https://github.com/user-attachments/assets/b3b51564-827e-4f1b-809c-e18b07553858)
  
- Lalu sesuaikan dengan yang ada digambar berikut:
  
  ![image](https://github.com/user-attachments/assets/4e920acf-051e-4ec4-8e30-5c36edfd10b0)
  
  kemudian klik save.
  
- kemudian klik panah ke bawah di sebelah account, lalu klik ‘add account or hardware wallet, lalu klik private keys, Kemudian isikan Private Keys yang didapatkan dari ganache, kemudian klik import.
  
  ![image](https://github.com/user-attachments/assets/8781ffcd-6e33-4f0e-8572-05757b45d4cc)

### 11. Kemudian klik ‘muat data sensor’ pada halaman Web3
kemudian akan muncul tampilan dari extension metamask. Kemudian klik connect maka tampilan dari Web3 akan menampilkan tabel dari data yang telah dikirimkan ke TCP Server

![image](https://github.com/user-attachments/assets/837e5c2f-75ef-4f23-b0f5-3a9c9f24d23c)
![image](https://github.com/user-attachments/assets/672e3ef9-96e1-4329-929d-2bb4532ccb5b)

### 12. Untuk membuka Database dari hasil bacaan sensor
- Buka http://localhost:8086 untuk menuju tampilan web InfluxDB
  ![image](https://github.com/user-attachments/assets/a69937d0-7dc7-4b39-b16f-fdc3923bc08c)
- Kemudian beri checklist pada
  1) From Bucket: ISIMonitor
  2) _measurement: monitoring
  3) _field: humidity dan temperature
  4) Filter:  i. location: Ruang Gudang Bahan Baku
             ii. sensor_id: SHT20-Gudang Tepung
            iii. stage: Tepung Gandum
  5) Kemudian atur Window Period menjadi:
     custom:5s, Aggregate Function Auto, kemudian bagian penampilan menjadi 'last', setelah itu Submit.

  ### Buka bagian 'script editor' dan salin semua query untuk bisa menampilkan grafik pada grafana

### 13. Untuk menghubungkan ke Grafana.
- Pastikan Grafana service sudah berjalan, dengan memasukkan command berikut
  ```
  sudo systemctl status grafana-server
  ```
- Lalu buka http://localhost:3000, dan tampilan web Grafana akan ditampilkan.
- Setelah itu ke bagian ‘Connections’ klik ‘add new connection’, kemudian pilih influxDB
- 
  ![Screenshot 2025-06-23 081001](https://github.com/user-attachments/assets/e70135ae-582c-4e6c-b635-770d205443c1)
  
- Klik ‘add new data source’
  
  ![image](https://github.com/user-attachments/assets/2456b78f-3665-4b45-a368-67b91b394c91)
  
- Kemudian dalam settingnya ubah Query Languagenya menjadi Flux, kemudian dalam HTTPnya masukkan URL dari InfluxDB
  
  ![image](https://github.com/user-attachments/assets/340b9639-24b1-4828-992a-60ef13d4634f)
  
- Kemudian pada Organization dan Token sesuaikan dengan yang ada dari InfluxDB dan profil InfluxDB. Kemudian klik Save & Test.
  
  ![image](https://github.com/user-attachments/assets/0435d0e4-cd73-4ab0-81dd-2172a093e13d)

- Setelah menambahkan data source pada step sebelumnya, kemudian kita menuju halaman ‘Dashboards’, lalu klik ‘Create dashboard’.
  
  ![image](https://github.com/user-attachments/assets/f856a5ad-a39d-4b75-a54a-30fd033e234f)

- Kemudian klik ‘Add visualization’
  
  ![image](https://github.com/user-attachments/assets/6eeb04a0-dba1-4ae6-b369-215cede1ba60)

- Pilih influx tadi sebagai data sourcenya
  
  ![image](https://github.com/user-attachments/assets/7ad596f0-6750-488f-80a3-5c46794af7e8)

- Lalu tempelkan salinan dari script query dari InfluxDB ke query di Visualisasi Grafana dan sesuaikan dengan data apa yang ingin ditampilkan
- Kemudian setelah penyesuaian selesai, klik Save Dashboard

### 14. Untuk membuka GUI menggunakan PyQT5
- Buka terminal dari Folder yang berisikan clone dari git ini, kemudian jalankan command
  ```
  python3 QTgui.py
  ```
  kemudian tekan enter, maka akan muncul tampilan seperti berikut
  
  ![image](https://github.com/user-attachments/assets/e343074a-cf9a-4a1d-aa11-ef80fff8ea7d)

## Tampilan Dashboard dan Hasil Data
### 1. Tampilan Dashboard dan Grafik pada InfluxDB
![image](https://github.com/user-attachments/assets/a69937d0-7dc7-4b39-b16f-fdc3923bc08c)
![image](https://github.com/user-attachments/assets/93fd7c13-d5fb-4e90-ace6-14e0e39da929)

Dapat dilihat pada Gambar pertama merupakan hasil tampilan grafik pembacaan sensor berdasarkan timestamp pada InfluxDB, dan pada Gambar kedua merupakan tabel data hasil pembacaan sensor berdasarkan timestamp pada InfluxDB. Dalam influxDB berguna untuk menyimpan data pembacaan sensor atau sebagai database yang dikirim dari mikrokontroler dan sensor melalui protokol TCP, lalu akan disimpan dalam struktur data berbasis waktu. Dalam sistem monitoring ini, influxDB yang bertanggung jawab merekam dan menyimpan data, setiap perubahan lingkungan sekecil apapun akan tercatat.

### 2. Hasil Tampilan Grafana
![image](https://github.com/user-attachments/assets/6610149e-95d9-4e0e-a40f-bbe815bd96d6)

Dapat dilihat pada Gambar merupakan hasil tampilan grafik pada grafana yang berperan sebagai interface visual untuk menampilkan data temperatur dan kelembaban secara real-time yang dikirim dari sensor SHT20 dari data yang ada dalam influxDB. Dashboard yang dibuat memberikan grafik time-series chart sehingga pengguna dapat melihat perubahan temperatur dan kelembapan selama proses penyimpanan berlangsung.

### 3. Blockchain dan Web3
![image](https://github.com/user-attachments/assets/fa2468fd-c8f3-46b4-b7ee-f36035859d14)

Pada Gambar pertama diatas merupakan hasil pembacaan tampilan data secara realtime dari sensor SHT20 yang diintegrasikan ke dalam sistem blockchain. Setiap data yang ditampilkan akan dicatat sebagai transaksi dalam jaringan blockchain. Integrasi blockchain pada sistem ini memberikan jaminan keaslian dan integritas data. 

![image](https://github.com/user-attachments/assets/0e370606-b83c-411d-b5d2-4839c948cad0)

Pada Gambar kedua diatas menunjukkan hasil visualisasi grafik temperatur dan kelembapan yang merupakan bagian dari sistem blockchain yang diintegrasikan ke Web3. Terdapat dua grafik yang tertera dalam satu tampilan, gari merah menunjukkan grafik temperatur dalam derajat celcius (°C), dan garis biru menunjukkan grafik kelembapan dalam  persentase (%). 

![image](https://github.com/user-attachments/assets/f90a71f7-1eb8-4d7a-8d5b-f894f19c706f)

Pada Gambar ketiga diatas menunjukkan fitur dari Web3 yang digunakan sebagai pengonfirmasian bahan baku yang sedang melakukan loading in ke gudang bahan baku, yang kemudian ketika dikonfirmasikan dengan menggunakan timestamp yang sesuai format maka akan menampilkan status dari data sensor dari waktu tersebut dan juga kontak untuk pengonfirmasian proses loading in barang.

### 4. Hasil Interface pada Qt
![image](https://github.com/user-attachments/assets/e343074a-cf9a-4a1d-aa11-ef80fff8ea7d)

Dalam Gambar pertama diatas merupakan tampilan hasil grafik tampilan pada software Qt, disini menampilkan data temperatur dan kelembapan yang dikirimkan dari influxDB secara lokal. Pada Qt ini, bertujuan untuk menampilkan dalam bentuk grafik sederhana yang mudah dipahami pengguna untuk melihat data historis dalam bentuk dua grafik terpisah, grafik merah merupakan grafik untuk tampilan data Suhu dan grafik biru merupakan grafik untuk tampilan data kelembaban. Penggunaan warna yang berbeda untuk tampilan grafik ini bertujuan untuk membantu membedakan dua parameter yang berbeda tersebut. Integrasi langsung dengan influxDB memastikan bahwa data selalu ter-update secara real-time.

![image](https://github.com/user-attachments/assets/5d0cea67-00d0-4691-8f23-45c86bb42b89)

Dalam Gambar kedua diatas menunjukkan tampilan Tabel Historis dari Qt yang berguna untuk menampilkan daya sensor suhu dan kelembaban. Dalam setiap tabel mencatat timestamp secara realtime, Suhu dalam °C dan kelembaban dalam %.
 






