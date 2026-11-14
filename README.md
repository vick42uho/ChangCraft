<p align="center">
  <img alt="ChangCraft Thai Edition" src="assets/app-icon/changcraft-master.png" width="128">
</p>

<h1 align="center">ChangCraft (Thai Edition)</h1>

<p align="center">
  <b>โปรแกรมอ่านและแก้ไขไฟล์ PDF ภาษาไทยประสิทธิภาพสูง พัฒนาด้วยภาษา Rust จากรากฐาน Clean-Room</b><br>
  จัดการเอกสาร PDF, แก้ไขข้อความและออปเจ็กต์ภาษาไทย, จัดวรรณยุกต์ซ้อน, รองรับฟอนต์ไทยแท้ (Angsana, Cordia, Browallia, Sarabun) และพิมพ์เอกสารคมชัดระดับสูง<br>
  พัฒนาเพิ่มเติมเพื่อรองรับภาษาไทยเต็มรูปแบบ โดย <b>Wick</b>
</p>

<p align="center">
  <a href="https://github.com/vick42uho/ChangCraft/releases/latest"><img alt="Latest Release" src="https://img.shields.io/github/v/release/vick42uho/ChangCraft?style=flat-square&color=12a58a"></a>
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-12a58a?style=flat-square">
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-0a7563?style=flat-square">
  <img alt="Platform: Windows x64 | Linux x64" src="https://img.shields.io/badge/platform-Windows%20x64%20%7C%20Linux%20x64-12a58a?style=flat-square">
  <img alt="Offline & Private" src="https://img.shields.io/badge/privacy-100%25%20offline-0a7563?style=flat-square">
</p>

---

### 📥 ดาวน์โหลดสำหรับ Windows และ Linux (Download)

#### 🌟 เวอร์ชันล่าสุด: v0.20.0 (Latest Release)
| แพลตฟอร์ม / รูปแบบ | คำอธิบาย | ลิงก์ดาวน์โหลด |
| :--- | :--- | :--- |
| **📦 Windows Installer (MSI)** | ตัวติดตั้งมาตรฐาน Windows x64 มี Shortcut และไอคอนช้างไทย ติดตั้งควบคู่กับ PdfCraft ต้นฉบับได้ | [**ดาวน์โหลด .msi (v0.20.0)**](https://github.com/vick42uho/ChangCraft/releases/download/v0.20.0/changcraft-0.20.0-windows-x64.msi) |
| **⚡ Windows Portable (ZIP)** | เวอร์ชันพกพา Windows x64 แตกไฟล์แล้วเปิด `changcraft.exe` ใช้งานได้ทันที ไม่ต้องติดตั้ง | [**ดาวน์โหลด .zip (v0.20.0)**](https://github.com/vick42uho/ChangCraft/releases/download/v0.20.0/changcraft-0.20.0-windows-x64-portable.zip) |
| **🐧 Linux Debian / Ubuntu (.deb)** | แพ็กเกจติดตั้งมาตรฐานสำหรับ Ubuntu / Debian x86_64 | [**ดาวน์โหลด .deb (v0.20.0)**](https://github.com/vick42uho/ChangCraft/releases/download/v0.20.0/changcraft-0.20.0-linux-amd64.deb) |
| **🐧 Linux Portable (.tar.gz)** | เวอร์ชันพกพาสำหรับ Linux x86_64 แตกไฟล์แล้วรัน `./changcraft` ได้ทันที | [**ดาวน์โหลด .tar.gz (v0.20.0)**](https://github.com/vick42uho/ChangCraft/releases/download/v0.20.0/changcraft-0.20.0-linux-x64.tar.gz) |

#### 📦 เวอร์ชันก่อนหน้า (Previous Releases)
- **v0.18.1**: [Windows MSI](https://github.com/vick42uho/ChangCraft/releases/download/v0.18.1/changcraft-0.18.1-windows-x64.msi) | [Windows Portable ZIP](https://github.com/vick42uho/ChangCraft/releases/download/v0.18.1/changcraft-0.18.1-windows-x64-portable.zip)
- **v0.11.0**: [Windows MSI](https://github.com/vick42uho/ChangCraft/releases/download/v0.11.0/changcraft-0.11.0-windows-x64.msi) | [Windows Portable ZIP](https://github.com/vick42uho/ChangCraft/releases/download/v0.11.0/changcraft-0.11.0-windows-x64-portable.zip)

---

### 🙏 เครดิตต้นฉบับ (Attribution & Credit)

> โครงการ **ChangCraft (Thai Edition)** พัฒนาต่อยอดมาจากโครงการโอเพนซอร์สต้นฉบับ [**PdfCraft**](https://github.com/storytold/pdfcraft.git) ของทีมงาน **Storytold / ArtCraft**
> ขอขอบคุณทีมงานต้นฉบับสำหรับสถาปัตยกรรม Clean-Room PDF Engine ในภาษา Rust ที่ยอดเยี่ยม ปลอดภัย และยึดมั่นในมาตรฐานไม่แครช (Never Crash)

---

### ✨ ฟีเจอร์เด่นของ ChangCraft (Highlights)

- 🐘 **เอกลักษณ์และไอคอนช้างไทย**: ปรับแต่งหน้าตาและไอคอนช้างไทย (Thai Royal Elephant) สวยงาม เป็นเอกลักษณ์
- ✍️ **การจัดวรรณยุกต์และสระภาษาไทยสมบูรณ์แบบ (HarfRust Shaping)**:
  - คำนวณตำแหน่งสระบน-ล่าง และวรรณยุกต์ซ้อน (เช่น *ปั้นจั่น, ผู้ใหญ่, กี่ปี่, น้ำ*) แม่นยำ สระไม่ลอย ไม่จม ไม่ทับกัน
  - แยกสระอำ (`ำ`) เป็น นิคหิต (`ํ`) + สระอา (`า`) ในระดับ PDF Engine ทำให้วัดความกว้างข้อความแม่นยำ ลบตัวอักษรได้ไม่เพี้ยน
  - ระบบตัดคำพจนานุกรมไทย (Thai Dictionary-based Line Wrap) สระหน้า (*เ, แ, โ, ใ, ไ*) เกาะติดพยัญชนะเสมอ
- 🔤 **รองรับฟอนต์ภาษาไทยแท้จากระบบ & Built-in**:
  - ดึงฟอนต์จาก Windows อัตโนมัติ: `Angsana New`, `Cordia New`, `Browallia New` ครบทั้ง **Bold (ตัวหนา)**, **Regular**, **Italic** รวมถึงเวอร์ชัน UPC, Tahoma และ Leelawadee
  - ฟอนต์ภาษาไทยมาตรฐานในตัว: `Sarabun (สารบรรณ)`, `Noto Sans Thai`, `IBM Plex Sans Thai`, `Charmonman`
- 📝 **แก้ไขและลากย้ายข้อความ/ออปเจ็กต์ได้ลื่นไหล**:
  - ลากย้ายย่อหน้าข้อความ, กล่องข้อความ และรูปภาพที่ถูก Clip ได้ลื่นไหลตามเมาส์แบบเรียลไทม์ ไม่ค้าง ไม่กระตุก
  - คัดลอก ตัด และวางออปเจ็กต์ข้ามหน้า/ข้ามเอกสาร พร้อมระบบเก็บประวัติ Undo/Redo ครบถ้วน
- 🖨️ **งานพิมพ์ความละเอียดสูง (High-Fidelity Print Pipeline)**:
  - พิมพ์ผ่าน Windows Spooler ในโหมดเวกเตอร์ คมชัดระดับ 600, 1200, 2400 DPI และโหมด Slice Tiling 300/600 DPI
- 🔍 **ระบบ Thai OCR สองระบบ**:
  - *Local PaddleOCR*: ถอดข้อความไทย-อังกฤษในเครื่องแบบออฟไลน์ 100% ไม่ต้องต่อเน็ต
  - *OpenTyphoon OCR*: รองรับการต่อ API Typhoon สำหรับถอดข้อความลายมือภาษาไทยที่ซับซ้อน
- 🛡️ **ความปลอดภัยและความเป็นส่วนตัวสูงสุด**: ทำงานแบบออฟไลน์ 100% ไม่มี Telemetry ไม่มีการส่งข้อมูลส่วนตัวออกนอกเครื่อง
