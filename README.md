# GridPrint A4

**GridPrint A4** je jednoduchá, rychlá a spolehlivá desktopová aplikace napsaná v jazyce Rust, která slouží k formátování a tisku jednoho obrázku několikrát na jedinou stránku formátu A4 bez okrajů. Program umožňuje snadné rozvržení snímků do mřížky a přímý tisk do Windows tiskové fronty.

## Hlavní funkce

*   **Volba rozložení:** Možnost tisku v mřížce **2x2** (4 obrázky), **3x3** (9 obrázků) nebo **4x4** (16 obrázků) na jednu stranu A4.
*   **Režimy přizpůsobení buněk:**
    *   *Vyplnit (Crop & Fill)* – Obrázek se automaticky ořízne a roztáhne tak, aby beze zbytku vyplnil buňku mřížky (vhodné pro bezokrajový vzhled).
    *   *Celý obrázek (Fit)* – Obrázek se zmenší tak, aby byl vidět celý bez ořezu, přičemž volná místa jsou doplněna bílým pozadím.
*   **Chytrá orientace:** Program automaticky detekuje poměr stran nahraného obrázku a přizpůsobí orientaci stránky (na šířku/výšku), s možností ručního přepsání.
*   **Nativní tisk na Windows:** Přímá integrace s Windows Spoolerem. Aplikace nastavuje orientaci a počet kopií na úrovni tiskového ovladače (DEVMODE).
*   **Věrný náhled:** Grafické rozhraní s hardwarovou akcelerací zobrazuje přesný náhled s ohraničením listu papíru a dělicími čarami mřížky.
*   **Export:** Možnost uložit výslednou A4 mřížku ve vysokém rozlišení (300 DPI) do souboru **PDF**, **PNG** nebo **JPEG**.

## Sestavení ze zdrojových kódů

Pro sestavení aplikace je vyžadován kompilátor Rust.

1.  Sestavte aplikaci v release módu:
    ```bash
    cargo build --release
    ```
2.  Spustitelný soubor naleznete v adresáři `target/release/gridprint.exe`.

## Sestavení pro Windows z Linuxu (Křížová kompilace)

Pokud kompilujete na Linuxu pro Windows (vyžaduje MinGW a target `x86_64-pc-windows-gnu`):
```bash
cargo build --release --target x86_64-pc-windows-gnu
```

## Licence

Tento projekt je licencován pod licencí GNU GPL v3. Podrobnosti naleznete v přiloženém souboru [LICENSE](LICENSE).
