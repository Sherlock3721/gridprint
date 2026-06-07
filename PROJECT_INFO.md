# GridPrint A4 — Kompletní informace o projektu

Aplikace **GridPrint A4** je vysoce optimalizovaný desktopový program naprogramovaný v jazyce **Rust**. Splňuje požadavek na vytvoření jediné, zcela samostatné binárky bez nutnosti instalovat Python nebo jiné externí závislosti.

Umožňuje uživateli vybrat jeden obrázek a vytisknout jej v mřížce (2x2, 3x3 nebo 4x4) bez okrajů na celou stranu formátu A4, s možností okamžitého hardwarově akcelerovaného náhledu a nastavení počtu kopií.

---

## 🚀 Hlavní vlastnosti a funkce

1. **Plně nativní spustitelný soubor (`.exe`):**
   - Zkompilováno přímo do strojového kódu pro Windows (x86_64).
   - Velikost po optimalizaci je pouze **4.6 MB** (obsahuje kompletní GUI, tiskový a PDF engine bez jakýchkoliv externích runtime závislostí).
2. **Výběr obrázku:**
   - Podpora standardních formátů (`.jpg`, `.jpeg`, `.png`, `.webp`, `.bmp`) přes dialogové okno.
   - Kliknutí na plátno náhledu rovněž otevře výběr souboru.
3. **Konfigurace rozložení:**
   - Rozložení mřížky: **2x2**, **3x3** nebo **4x4** na jednu stranu A4.
4. **Možnosti přizpůsobení:**
   - **Vyplnit (Crop & Fill):** Obrázek se inteligentně ořízne a roztáhne tak, aby beze zbytku vyplnil buňku mřížky (vhodné pro bezokrajový tisk).
   - **Celý obrázek (Fit):** Obrázek se zmenší tak, aby byl vidět celý v buňce, případné nesrovnalosti poměru stran jsou doplněny bílým pozadím.
5. **Orientace:**
   - Automaticky detekuje orientaci nahraného obrázku a nastaví podle toho orientaci stránky A4 (Na šířku / Na výšku).
   - Možnost manuálního přepnutí.
6. **Počet kopií:**
   - Nativní číselný spinner pro nastavení počtu tištěných stránek A4.
7. **Pokročilý tiskový náhled:**
   - Využívá knihovnu `egui` s plnou hardwarovou akcelerací.
   - Realistické stínování okrajů papíru A4 a vykreslení jemných dělicích čar mřížky.
   - Dynamicky škáluje náhled podle velikosti okna.
8. **Nativní tisk (Windows Spooler):**
   - Komunikuje přímo se službou tiskové fronty systému Windows přes GDI API.
   - Přepíná orientaci papíru a počet kopií na úrovni tiskového ovladače (`DEVMODE`).
9. **Záložní export:**
   - Možnost přímého exportu výsledné mřížky do **PDF** (pomocí `printpdf` se zachováním 300 DPI rozměrů) nebo do obrázku **PNG/JPG**.
   - Na operačních systémech mimo Windows (např. při testování v Linuxu) aplikace automaticky přepne tisk na export do PDF.
10. **Moderní vzhled:**
    - Čistý Dark/Light režim s přepínačem přímo v ovládacím panelu.

---

## 📂 Souborová struktura projektu

- [Cargo.toml](file:///home/evolve/gridprint/Cargo.toml) — Definice projektu, závislostí a optimalizací pro release.
- [Cargo.lock](file:///home/evolve/gridprint/Cargo.lock) — Uzamčené verze všech závislostí.
- [src/main.rs](file:///home/evolve/gridprint/src/main.rs) — Zdrojový kód aplikace v Rustu.
- [gridprint.exe](file:///home/evolve/gridprint/gridprint.exe) — Výsledná optimalizovaná binárka pro Windows.
- [README.md](file:///home/evolve/gridprint/README.md) — Základní uživatelská dokumentace.
- [LICENSE](file:///home/evolve/gridprint/LICENSE) — Licenční ujednání (GNU GPL v3).
- [PROJECT_INFO.md](file:///home/evolve/gridprint/PROJECT_INFO.md) — Tento dokument s kompletními informacemi o projektu.

---

## 🛠️ Architektura a struktura kódu

Kód v [src/main.rs](file:///home/evolve/gridprint/src/main.rs) je rozdělen do několika logických bloků:

### 1. Platformově specifický tisk (`win_print`)
Modul je aktivní pouze na operačním systému Windows (`#[cfg(target_os = "windows")]`).
* **`list_printers`**: Dotazuje se Windows Spooleru přes GDI API na seznam dostupných tiskáren.
* **`print_image`**: Otevře zvolenou tiskárnu, nakonfiguruje tiskové vlastnosti (`DEVMODE` - orientace papíru A4 a počet kopií) a pošle raw bitmapový buffer pomocí `StretchDIBits` přímo na kontext tiskárny (DC).

### 2. Správa stavu a rozhraní (`GridPrintApp`)
Hlavní struktura implementující rozhraní `eframe::App` pro knihovnu `egui`.
* **`load_selected_image`**: Asynchronně načítá obrázek a automaticky nastavuje orientaci.
* **`update_preview_texture`**: Generuje výsledný bitmapový obraz mřížky a nahrává ho do GPU paměti jako texturu pro zobrazení v reálném čase.
* **`update`**: Vykresluje ovládací panel (výběr rozvržení, režimu přizpůsobení, orientace, počtu kopií, tlačítek tisk/export) a samotnou plochu náhledu papíru A4 s vrženým stínem.

### 3. Zpracování obrazu (`generate_grid_image`)
Matematické jádro, které sestavuje finální obrázek mřížky pro tisk a export. Počítá přesné rozměry buněk a ořezy tak, aby nevznikaly zaokrouhlovací 1px mezery mezi jednotlivými obrázky. Používá kvalitní Lanczos3 filtr pro škálování.

### 4. Generování PDF (`save_grid_pdf`)
Sestavuje výsledný PDF dokument o velikosti A4 s vloženou bitmapou ve vysoké hustotě pixelů (300 DPI) za použití knihovny `printpdf`.

---

## 🔧 Postup sestavení (Kompilace)

Kompilace probíhá pomocí standardního správce balíčků `cargo`.

### Křížová kompilace z Linuxu pro Windows:
Prostředí je předpřipravené pro křížovou kompilaci pomocí GNU toolchainu:
```bash
cargo build --release --target x86_64-pc-windows-gnu
```
Výstupní soubor se vytvoří v: `target/x86_64-pc-windows-gnu/release/gridprint.exe`.

### Sestavení přímo ve Windows:
```cmd
cargo build --release
```
Výstupní soubor se vytvoří v: `target\release\gridprint.exe`.

---

## ⚡ Optimalizace velikosti binárního souboru

Ve výchozím nastavení Rust kompiluje binární soubory se všemi knihovnami staticky a ponechává v nich velké množství debugovacích symbolů a režijního kódu pro paniku (unwind). Původní velikost souboru činila **14.8 MB**.

V [Cargo.toml](file:///home/evolve/gridprint/Cargo.toml) byly aplikovány následující optimalizace:

* **`opt-level = "z"`**: Optimalizuje kód prioritně pro minimální velikost binárního souboru.
* **`lto = true`**: Aktivuje Link-Time Optimization, čímž umožní linkeru odstranit veškerý nepoužívaný kód (dead code) z celého grafu závislostí.
* **`codegen-units = 1`**: Slučuje kompilaci do jediné jednotky, což LLVM umožňuje provést hlubší a celistvější optimalizace kódu.
* **`panic = "abort"`**: Nahrazuje složitý mechanismus rozbalování zásobníku (unwinding) při havárii okamžitým ukončením procesu, čímž eliminuje velký objem pomocných tabulek.
* **`strip = true`**: Odstraňuje všechny tabulky symbolů a debugovací informace z výsledného `.exe` souboru.

Díky této konfiguraci má výsledný soubor pouze **4.6 MB**.
