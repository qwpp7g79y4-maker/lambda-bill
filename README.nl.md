# lambda-bill

[![CI](https://github.com/qwpp7g79y4-maker/lambda-bill/actions/workflows/ci.yml/badge.svg)](https://github.com/qwpp7g79y4-maker/lambda-bill/actions/workflows/ci.yml)

Schat de on-demand-kosten van AWS Lambda uit CloudWatch-`REPORT`-regels, per functie, voor x86_64 en arm64, met een advies voor het geheugen. Voor wie een Lambda-logexport heeft en de kosten tegen lijstprijzen en een geheugenadvies wil, zonder account en zonder netwerkverbinding. De Engelse versie is [README.md](README.md); beide hebben dezelfde inhoud.

De tests en de gemeten getallen staan in `QUALITY.md`.

- Het leest de bestanden, mappen en standaardinvoer die je noemt. Het schrijft het rapport naar standaarduitvoer, of naar het bestand dat je met `--output` geeft. Verder verandert het niets.
- Geen netwerk, geen AWS SDK, geen account, geen token, geen betaalde dienst.
- De tarieven zijn als momentopname ingebouwd, of komen uit een `--prices`-bestand dat jij geeft.
- Het resultaat is een schatting uit lijstprijzen, niet je AWS-factuur.

## Waarom

Een Lambda-logexport heeft al de twee getallen die de on-demand-factuur gebruikt: de gefactureerde duur en het ingestelde geheugen, per aanvraag. lambda-bill telt die op je eigen machine bij elkaar op, per functie, voor x86_64 en arm64, en kijkt naar Max Memory Used om een andere geheugeninstelling voor te stellen. Het roept AWS niet aan en heeft geen account nodig.

## Installatie

Met Rust 1.85 of nieuwer, vanuit een kopie van deze repository:

```sh
cargo install --locked --path .
lambda-bill --help
```

Deze crate is niet gepubliceerd. Installeer vanuit deze kopie, niet vanaf crates.io. Er is nog geen releasearchief: `.github/workflows/release.yml` bouwt archieven met de licentieteksten van derden zodra een versietag wordt gepusht, en er is nog geen tag gepusht.

**Ondersteunde systemen.** De testsuite is alleen op Linux x86_64 gedraaid. Een releasebuild daar vraagt glibc 2.34 of nieuwer; de workflow heeft een job die het hoogste glibc-symbool van een releasebuild op Ubuntu 22.04 controleert, en die is nog niet gedraaid. De code is ook gecompileerd en met clippy gecontroleerd voor de doelen Windows (`x86_64-pc-windows-gnu`), macOS (`x86_64-apple-darwin`) en musl (`x86_64-unknown-linux-musl`), maar op die systemen is geen test gedraaid. Een standaarduitvoer die bij de start gesloten of alleen-lezen is, wordt alleen op Linux herkend.

De suite heeft 273 tests (`cargo test --locked`), allemaal geslaagd. [QUALITY.nl.md](QUALITY.nl.md) heeft de tabel.

## Snelstart

Twee REPORT-regels voor één functie. Dit schrijft ze naar een nieuwe tijdelijke map (de repository blijft onaangeroerd) en rekent ze door:

```sh
dir=$(mktemp -d)
cat > "$dir/report.log" <<'LOG'
# lambda-bill-function: checkout
# lambda-bill-arch: x86_64
REPORT RequestId: 11111111-1111-1111-1111-111111111111	Duration: 100.00 ms	Billed Duration: 100 ms	Memory Size: 512 MB	Max Memory Used: 40 MB	Init Duration: 200.00 ms	
REPORT RequestId: 22222222-2222-2222-2222-222222222222	Duration: 50.10 ms	Billed Duration: 51 ms	Memory Size: 512 MB	Max Memory Used: 64 MB	
LOG
lambda-bill "$dir/report.log"
```

Dezelfde bytes staan in `examples/sample-report.log` van een kopie, dus `lambda-bill examples/sample-report.log` geeft hetzelfde rapport. Een test draait elk consoleblok in het Engelse bestand en vergelijkt het met dit programma (met `--as-of 2026-11-20`, zodat de controle op de prijsleeftijd niet van de klok afhangt):

```console
$ lambda-bill examples/sample-report.log
prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
provisioned concurrency not included: only on-demand duration and requests are priced
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)
headroom: 20%

function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds      duration_usd  request_usd         total_usd  advice
checkout  x86_64              512           64         2            1      0.0755  0.00000125833585    0.0000004  0.00000165833585  downsize to 128 MB
total                                                  2            1      0.0755  0.00000125833585    0.0000004  0.00000165833585

estimated_usd=0.00000165833585
advice checkout x86_64 512 MB: downsize to 128 MB; duration USD 0.00000125833585 -> 0.0000003145839625 if billed duration stays the same
note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

512 MB gedurende 151 gefactureerde milliseconden is 0,0755 GB-seconden. Een van de twee regels heeft `Init Duration`, dus er is één cold start. De inittijd wordt niet nog eens opgeteld: die zit al in `Billed Duration`. Max Memory Used is 64 MB, dus het advies is het geheugen op 128 MB te zetten. Het dollarverschil op die regel gaat ervan uit dat de gefactureerde duur gelijk blijft.

## Aan de slag in 5 minuten

1. Installeer vanuit een kopie van deze repository: `cargo install --locked --path .` (Rust 1.85 of nieuwer).
2. Haal een logexport op, bijvoorbeeld met `aws logs filter-log-events` of via de CloudWatch-console, en bewaar die als bestand of map.
3. Draai `lambda-bill export.log`. Staat de functienaam niet in het bestand, geef die dan mee met `--function naam`. Is de functie arm64, voeg `--arch arm64` toe.
4. Lees de tabel: GB-seconden, bedrag per functie, en het advies voor het geheugen. De uitvoer is een schatting uit lijstprijzen, niet je AWS-factuur.
5. Wil je de cijfers verder gebruiken, voeg dan `--json` of `--md` toe, of schrijf met `--output rapport.json` naar een bestand.

## Opties

```text
lambda-bill [OPTIES] <BESTAND>...
```

`BESTAND` is een logbestand, een gzip-export (herkend aan de bytes, niet aan de naam), een map (in gesorteerde volgorde doorlopen) of `-` voor standaardinvoer. Invoer is CloudWatch-tekstregels met `REPORT`, CloudWatch-export-JSON (een document of een gebeurtenis per regel) en gzip daarvan. De vormen staan onder Hoe het telt (C19, C23, C29).

| Optie | Betekenis | Standaard |
| --- | --- | --- |
| `--prices BESTAND` | Prijsbestand dat de ingebouwde tarieven vervangt (C28) | ingebouwde eerste-tierlijst |
| `--arch ARCH` | Architectuur als het bestand die niet noemt: `x86_64` of `arm64` | `x86_64` |
| `--function NAAM` | Functienaam als bestand of pad er geen noemt | geen |
| `--functions BESTAND` | JSON-object van functienaam naar architectuur | geen |
| `--headroom PERCENT` | Procent boven de geheugenbasis voor het advies, een geheel getal van 0 tot 100 | 20 |
| `--as-of DATUM` | Datum (`JJJJ-MM-DD`) voor de controle op de prijsleeftijd | vandaag (UTC) |
| `--quiet` | Alleen het resultaat en fouten, geen waarschuwing op standaardfout | uit |
| `--verbose` | Ook voortgang op standaardfout | uit |
| `--output BESTAND` | Schrijf het rapport naar dit bestand in plaats van standaarduitvoer | standaarduitvoer |
| `--json` | JSON in plaats van de teksttabel | uit |
| `--md` | Markdown-tabel in plaats van de teksttabel (niet met `--json`) | uit |
| `-h`, `--help` | Toon hulp | |
| `-V`, `--version` | Toon de versie (een debugbuild voegt de commit toe) | |

`lambda-bill --help` toont dezelfde lijst (in het Engels) en `man/lambda-bill.1` is de handleiding.

## Voorbeeld

Een arm64-functie heeft geen architectuur in de REPORT-regel. Geef die mee:

```console
$ lambda-bill --arch arm64 --function worker examples/arm-report.log
prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
provisioned concurrency not included: only on-demand duration and requests are priced
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)
headroom: 20%

function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds      duration_usd  request_usd         total_usd  advice
worker    arm64               128           20         1            0     0.00125  0.00000001666675    0.0000002  0.00000021666675  keep
total                                                  1            0     0.00125  0.00000001666675    0.0000002  0.00000021666675

estimated_usd=0.00000021666675
note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

Dezelfde architectuur kan uit een map van functienaam naar `x86_64` of `arm64` komen. De map geeft de functie geen naam; `--function` doet dat nog steeds, tenzij het bestand of het pad al een naam heeft.

```console
$ lambda-bill --function worker --functions examples/functions.json examples/arm-report.log
prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
provisioned concurrency not included: only on-demand duration and requests are priced
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)
headroom: 20%

function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds      duration_usd  request_usd         total_usd  advice
worker    arm64               128           20         1            0     0.00125  0.00000001666675    0.0000002  0.00000021666675  keep
total                                                  1            0     0.00125  0.00000001666675    0.0000002  0.00000021666675

estimated_usd=0.00000021666675
note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

Een CloudWatch-exportmap geeft de functie een naam met het pad `aws/lambda/<naam>`. `examples/export` heeft `billing` en `checkout`:

```console
$ lambda-bill examples/export
prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
provisioned concurrency not included: only on-demand duration and requests are priced
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)
headroom: 20%

function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds       duration_usd  request_usd          total_usd  advice
billing   x86_64              256           30         1            0     0.00125  0.000000020833375    0.0000002  0.000000220833375  downsize to 128 MB
checkout  x86_64              256           30         1            0     0.00125  0.000000020833375    0.0000002  0.000000220833375  downsize to 128 MB
total                                                  2            0      0.0025   0.00000004166675    0.0000004   0.00000044166675

estimated_usd=0.00000044166675
advice billing x86_64 256 MB: downsize to 128 MB; duration USD 0.000000020833375 -> 0.0000000104166875 if billed duration stays the same
advice checkout x86_64 256 MB: downsize to 128 MB; duration USD 0.000000020833375 -> 0.0000000104166875 if billed duration stays the same
note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

`--json` toont dezelfde totalen (en `schema_version`, de eerste sleutel), met geldbedragen als tekst. De bron-URL en de datum staan in het document:

```console
$ lambda-bill --json --arch arm64 --function worker examples/arm-report.log
{
  "schema_version": 1,
  "version": "1.0.0",
  "currency": "USD",
  "source": "https://aws.amazon.com/lambda/pricing/",
  "date": "2026-11-09",
  "region": "us-east-1",
  "tier": "first",
  "estimate": true,
  "estimate_note": "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)",
  "free_tier_applied": false,
  "provisioned_concurrency_included": false,
  "headroom_percent": 20,
  "advice_note": "advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.",
  "warnings": [],
  "functions": [
    {
      "function": "worker",
      "architecture": "arm64",
      "memory_mb": 128,
      "max_memory_used_mb": 20,
      "requests": 1,
      "cold_starts": 0,
      "gb_seconds": "0.00125",
      "duration_usd": "0.00000001666675",
      "request_usd": "0.0000002",
      "total_usd": "0.00000021666675",
      "advice": "keep",
      "advice_mb": 128,
      "duration_usd_if_duration_stays": "0.00000001666675"
    }
  ],
  "totals": {
    "requests": 1,
    "cold_starts": 0,
    "gb_seconds": "0.00125",
    "duration_usd": "0.00000001666675",
    "request_usd": "0.0000002",
    "total_usd": "0.00000021666675",
    "estimated_usd": "0.00000021666675"
  }
}
```

`--md` toont dezelfde bedragen als Markdown-tabel. Een `|`, een backslash of een regeleinde in een cel wordt geëscaped, en de regel met de schatting staat er ook. `--md` en `--json` kunnen niet samen worden gebruikt.

```console
$ lambda-bill --md --arch arm64 --function worker examples/arm-report.log
# Lambda cost estimate

Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

- prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
- free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
- provisioned concurrency not included: only on-demand duration and requests are priced
- headroom: 20%

### Cost

| function | architecture | memory_mb | max_used_mb | requests | cold_starts | gb_seconds | duration_usd | request_usd | total_usd | advice |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| worker | arm64 | 128 | 20 | 1 | 0 | 0.00125 | 0.00000001666675 | 0.0000002 | 0.00000021666675 | keep |
| total |  |  |  | 1 | 0 | 0.00125 | 0.00000001666675 | 0.0000002 | 0.00000021666675 |  |

estimated_usd=0.00000021666675

note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

`examples/README.md` heeft vier opdrachten met de verwachte uitvoer.

## Hoe het telt

C1. Eén `REPORT`-regel is één aanvraag. `START`, `END` en andere regels zijn geen aanvragen.

C2. GB-seconden zijn `memory_mb * billed_ms / 1024 / 1000`. 512 MB gedurende 151 ms is 0,0755 GB-seconden.

C3. Het veld Duration wordt niet voor de kosten gebruikt. Alleen Billed Duration telt.

C4. Init Duration wordt niet bij Billed Duration opgeteld. Het markeert alleen een cold start als het het veld direct na Max Memory Used is. De inittijd zit al in Billed Duration.

C5. Aanvragen kosten 0,20 USD per miljoen op x86_64 en op arm64.

C6. De duur op x86_64 kost 0,0000166667 USD per GB-seconde. De vorm van de prijspagina, 3 miljoen aanvragen van 120 ms op 1536 MB, is 540000 GB-seconden en 9,000018 USD duur, plus 0,60 USD aan aanvragen.

C7. De duur op arm64 kost 0,0000133334 USD per GB-seconde. Dat is niet precies 80 procent van het x86_64-tarief: 10000 GB-seconden kosten 0,166667 USD op x86_64 en 0,133334 USD op arm64, niet 0,1333336.

C8. Elk rapport noemt de valuta, de bron-URL, de datum, het regiolabel en de tier. De ingebouwde opgave is USD, `https://aws.amazon.com/lambda/pricing/`, 2026-11-09, `us-east-1`, `first`.

C9. De gratis tier van het account wordt niet afgetrokken. Die is 1000000 aanvragen en 400000 GB-seconden per maand, gedeeld over architecturen. Tegen deze tarieven is die hoeveelheid 6,86668 USD. Deze export is niet het hele account, dus een klein rapport kost nog steeds meer dan 0,00 USD.

C10. Provisioned Concurrency wordt niet geprijsd. Elk rapport zegt dat het niet is inbegrepen. Het programma past geen Provisioned-Concurrency-duurtarief of -aanvraagtarief toe.

C11. Een verkleiningsdoel is `ceil(max used * (100 + headroom) / 100)`, dan minstens 128 MB en hoogstens 10240 MB. Het wordt alleen gebruikt als dat doel onder de ingestelde grootte ligt. Headroom 20 bij 200 MB gebruikt op 256 MB stelt 240 MB voor. Headroom 0 bij 400 MB gebruikt op 512 MB stelt 400 MB voor. 40 MB gebruikt op 512 MB stelt 128 MB voor.

C12. Vanaf 90 procent van de ingestelde grootte is het verhogingsdoel `ceil(ingesteld * (100 + headroom) / 100)`, minstens één megabyte boven de huidige grootte en hoogstens 10240 MB. Max Memory Used is niet de basis, omdat het geen piek boven de instelling kan tonen. 900 van 1000 verhoogt naar 1200. 899 van 1000 blijft. 231 van 256 verhoogt naar 308. 230 van 256 blijft. 200 gebruikt op een functie van 128 MB verhoogt naar 154. 10240 blijft als het niet kan groeien. Als deze regel en een verkleining allebei kunnen gelden, wint verhogen: de verkleiningsregel geldt niet vanaf 90 procent. 117 MB gebruikt van 129 MB met headroom 5 verhoogt naar 136, geen verkleining naar 128.

C13. Advies-dollars gaan ervan uit dat de gefactureerde duur gelijk blijft. De noot zegt dat dit geen voorspelling is. Een keep-rij heeft geen adviesregel.

C14. 1024 MB gedurende 1000 ms is 1 GB-seconde. 1000 MB gedurende 1000 ms niet. De prijspagina gebruikt 1 GB = 1024 MB.

C15. Eén rij is één functie, één architectuur en één ingestelde geheugengrootte. Rijen staan op functie, dan arm64 vóór x86_64, dan geheugen. Rapporten met dezelfde sleutel worden opgeteld, en Max Memory Used is de hoogste ervan.

C16. Een RequestId telt één keer per functie. De eerste kopie wint, in de volgorde van de paden die je geeft. Een map wordt in gesorteerde padvolgorde doorlopen. De waarschuwing zegt hoeveel duplicaten zijn overgeslagen en toont de id niet. Dezelfde id bij twee functies telt twee keer.

C17. Het programma heeft geen AWS SDK en opent geen netwerkverbinding. Het leest alleen de bestanden die je geeft. De ingebouwde tarieven zijn ingecompileerd uit `prices/lambda-ondemand.json`.

C18. De standaardarchitectuur is x86_64. Dat tarief is hoger dan arm64, dus een onbekende architectuur wordt niet te laag geprijsd. Een `--functions`-regel wint van die standaard en moet kloppen met een architectuur die in het bestand staat. Alleen de namen `x86_64` en `arm64` worden geaccepteerd.

C19. De functienaam komt uit de loggroep (`/aws/lambda/<naam>` of `aws/lambda/<naam>`), dan uit een kop (`# lambda-bill-function:` of `# function:`), dan uit een pad `aws/lambda/<naam>`, dan uit `--function`. Twee verschillende gevonden namen zijn een fout. `--function` overschrijft een gevonden naam niet. Een ontbrekende naam is een fout. Een naam is 1 tot 64 tekens uit `A-Z`, `a-z`, `0-9`, `_` en `-`.

C20. Het ingestelde geheugen moet van 128 tot en met 10240 MB zijn. Max Memory Used mag van 0 tot en met 10240 zijn, ook een waarde boven de ingestelde grootte. Een gefactureerde duur van 0 wordt geaccepteerd. De getalvelden worden begrensd door hun lengte in tekens, niet door hun waarde: Billed Duration mag tot 20 tekens hebben (hoogstens 18446744073709551615, dus `00000000000000000100` is 100 en `000000000000000000100` wordt geweigerd), Memory Size en Max Memory Used tot 10 tekens (`0000000128` is 128). De Billed Duration van de aanvragen van één rij wordt in 64 bits opgeteld: een som boven 18446744073709551615 is de fout `billed duration overflow`.

C21. Gzip wordt herkend aan de bytes `1f 8b`, niet aan de bestandsnaam. Een tekstbestand met de naam `.gz` wordt als tekst gelezen. Elk lid van een gzip met meerdere leden wordt gelezen. Bytes na een lid die geen geldig volgend lid zijn, zijn een fout. Beschadigde gzip is een fout.

C22. Aanvraag-id's worden niet getoond.

C23. Een regel die `REPORT RequestId:` bevat en niet te lezen is, is een fout, en de tekst van de regel wordt niet getoond. Een regel die met `{` begint en geen JSON is, is ook een fout, zodat hij de rekening niet ongezien kan verlagen. Een JSON-regel die een REPORT bevat maar geen aanvraag oplevert, is een fout. Een JSON-object met zowel `message` als `events` is een fout, ook binnen een document of binnen een andere gebeurtenis. In een JSON-document of JSON-regel is elke `REPORT RequestId:`-tekst die niet als aanvraag wordt gelezen ook een fout, bijvoorbeeld een tweede REPORT in `@message` naast `message` (`document: not a Lambda REPORT line`). De teksten en de sleutels worden doorzocht nadat de JSON-escapes zijn ontcijferd, dus `REPORT RequestId:` wordt ook gevonden.

C24. Het totaal is de exacte som van de rijbedragen. Een bedrag of een som dat geen exact decimaal getal is, is een fout. Er wordt niet naar een dag of een maand geschaald. Het rapport zegt niet "per dag" en toont geen `factor:`. De woorden "per month" komen alleen in de zin over de gratis tier voor.

C25. JSON begint met `schema_version` (het gehele getal 1, de versie van de indeling, niet van het programma), en bevat dan de versie, de valuta, de bron-URL en de datum, in die volgorde. Geld en GB-seconden zijn tekst. `estimate` is true, met de zin in `estimate_note`, en `totals.estimated_usd` herhaalt `totals.total_usd`. `free_tier_applied` is false. Het document eindigt met een regeleinde.

C26. Een REPORT-regel heeft `REPORT RequestId:` op een witruimtegrens nodig, dan Duration, Billed Duration, Memory Size en Max Memory Used, in die volgorde. Een tweede markering op dezelfde regel is een fout. `XREPORT` komt niet overeen: op een gewone tekstregel wordt het genegeerd. Een markering vastgeplakt aan een byte-order-mark, een onzichtbaar teken met nulbreedte of een andere onzichtbare of niet-ASCII-spatie is een fout, geen stille overslag. Binnen een JSON- of haakjesregel is elke `REPORT RequestId:`-tekst die niet als aanvraag wordt geteld een fout, `XREPORT` inbegrepen. Trailing `XRAY`-tekst wordt genegeerd. Een veld dat na de vier vaste velden wordt herhaald, is ook trailing tekst: de eerste `Billed Duration` telt en een tweede wordt niet gelezen.

C27. De eenheden moeten `ms` en `MB` zijn. Billed Duration is een geheel getal. Duration en Init Duration mogen een breuk zijn.

C28. Een `--prices`-bestand vervangt de ingebouwde tarieven. Onbekende velden worden genegeerd. Een geldtekst mag alleen met spatie, tab, CR of LF zijn opgevuld. Andere Unicode-spaties worden geweigerd. Een geldwaarde met meer decimalen dan een decimaal getal van 28 cijfers kan bevatten, wordt geweigerd. De brontekst en de labels mogen geen stuurtekens, regel- of alinea-scheidingstekens of bidi-tekens bevatten. Een dubbele sleutel op het hoogste niveau wordt geweigerd. Een geneste dubbele sleutel is toegestaan. Een negatief getal en een getal met exponent worden geweigerd. Het bestand legt zijn eigen bron-URL en datum vast.

C29. Een tijdstempel vóór `REPORT` wordt genegeerd. Een tijdstempel tussen haakjes, een BOM aan het begin van het bestand en CRLF worden ook genegeerd. Een regel die met een ander onzichtbaar teken begint (een BOM midden in een bestand, een spatie met nulbreedte, een bidirectioneel teken), is een fout, ook voor een kop of een JSON-regel. Een JSON-logregel wordt gelezen via de tekst van `message`, `@message` of `Message`. Een bestand dat één JSON-object met `@message` of `Message` is, wordt ook gelezen, ook als het over meerdere regels is afgedrukt. Zo'n object noemt zijn functie en architectuur met `logGroup`, `logGroupName` en `architecture`, net als een gebeurtenis met `message`, en een foute waarde is een fout. Een `logGroup` en een `logGroupName` die verschillende groepen noemen, zijn een fout (`logGroup and logGroupName differ`).

C30. Een fout over een prijsbestand of een logregel noemt het probleem en bevat niet de inhoud van het bestand. Een stuurteken, regel- of alinea-scheidingsteken of bidi-teken in een pad wordt U+FFFD. In een argument dat clap weigert wordt het als `\u{..}` geschreven, zodat een bestandsnaam of een argument de terminalregel niet kan herschrijven. De regeleinden die clap zelf schrijft blijven: een onbekende optie of een ontbrekend bestandsargument toont meerdere regels, eindigend met een regeleinde. Hetzelfde geldt voor een argument dat geen geldige UTF-8 is, wat clap meldt zonder het te herhalen.

## Afsluitcodes

| Code | Betekenis |
| ---: | --- |
| 0 | Het rapport is geschreven. Waarschuwingen gaan naar standaardfout en veranderen de code niet. |
| 2 | Een invoer- of gebruiksfout: een fout bestand, regel, prijsbestand, datum of optie, een bedrag dat niet in een exact decimaal getal past (`price overflow`), een run met meer dan 100000 functie- en geheugenrijen (`more than 100000 function and memory rows`), of een standaarduitvoer die gesloten of alleen-lezen is of niet beschreven kan worden. Er wordt niets op standaarduitvoer gedrukt. |
| 3 | `--quiet` samen met `--verbose`. |
| 130 | SIGINT (Ctrl-C). Er wordt niets op standaarduitvoer gedrukt. |

Een panic vervangt deze codes nooit: een gesloten of volle standaarduitvoer is afsluitcode 2, geen crash. Het onderdeel Beperkingen heeft de details.

## Beperkingen

Alleen de on-demand-duur en -aanvragen van Lambda worden geprijsd. Provisioned Concurrency, Managed Instances, MicroVMs, Web Functions, tijdelijke opslag, tenant-isolatie, bewerkingen van durable functions, HTTP-responsstreaming, dataverkeer en SnapStart buiten wat al in Billed Duration zit, worden niet geprijsd. Een REPORT-regel zegt niet welke daarvan golden. Het programma gebruikt altijd de on-demand-tarieven en zegt dat Provisioned Concurrency niet is inbegrepen.

De gratis tier wordt niet afgetrokken. Eén export is niet de maand van het account. Er is geen extrapolatie per dag of per maand van de rijen die je geeft.

De standaardarchitectuur is x86_64 als het bestand er niets over zegt. Dat overschat de kosten van een arm64-functie, wat de veilige kant is. Geef `--arch` of `--functions` mee als je het weet.

Advies is geen voorspelling. Een kleinere geheugeninstelling betekent ook minder CPU, dus de functie kan langer lopen dan de dollars in de adviesregel doen vermoeden.

Eén REPORT-regel is één aanvraag. Het programma splitst of voegt geen asynchrone payload-brokken samen.

Een bestand groter dan 268435456 bytes wordt geweigerd, en ook een `--prices`- of `--functions`-bestand. Die twee moeten UTF-8 zijn en gewone bestanden: een pipe of een apparaat, zoals `<(opdracht)` of een FIFO, is `not a file`, zodat een pad dat nooit eindigt de run niet kan laten hangen. Een regel langer dan 1048576 bytes wordt geweigerd. Een JSON-document boven 4000000 waarden wordt ook geweigerd (zie hieronder). De map die je noemt is niveau 1. Niveau 64 wordt gelezen. Niveau 65 is een fout (`directory is nested too deeply`). Een symlink die je noemt wordt gevolgd. Een symlink die tijdens het doorlopen van een map wordt gevonden, wordt overgeslagen, net als een lid dat geen UTF-8 is of geen REPORT-regel heeft, met een waarschuwing. Dezelfde gevallen op een expliciet pad zijn fouten. Beschadigde gzip is altijd een fout.

Aanvraag-id's en de tekst van een geweigerde regel worden niet getoond.

Tekst vóór een JSON-regel maakt van de regel tekst, en dan zit de markering vast aan een aanhalingsteken en telt niet mee (C26). Een bestand met `{"message":"REPORT RequestId: a1 ..."}` op regel 1 en `2026-11-19T10:00:00Z {"message":"REPORT RequestId: a2 ..."}` op regel 2 geeft één aanvraag en afsluitcode 0. Hetzelfde geldt voor een byte-order-mark, een komma of `data: ` vóór de `{`. Haal zo'n voorvoegsel weg voordat je het bestand geeft. Een tijdstempel vóór een gewone `REPORT`-regel is goed (C29).

Een `# function:`-kop geldt voor het hele bestand, ook voor de REPORT-regels erboven. Twee verschillende koppen zijn een fout.

Een RequestId die twee keer voor één functie voorkomt telt één keer en de eerste kopie wint, ook als de tweede kopie andere geheugen- of duurwaarden heeft. De waarschuwing telt alleen de duplicaten.

Een bestand van 268435456 lege regels wordt in ongeveer 3 s en 526208 KB gelezen (gemeten met een releasebuild): het bestand zit twee keer in het geheugen, één keer als bytes en één keer als tekst.

Het JSON-logformaat van Lambda wordt niet gelezen. Een record als `{"type":"platform.report","record":{...}}` heeft geen `message`, dus het wordt zonder bericht overgeslagen en zijn aanvraag wordt niet geteld. Gebruik de `REPORT`-tekstregel. Een opgemaakt JSON-object dat geen CloudWatch-gebeurtenis is en geen `@message` of `Message` heeft, wordt geweigerd als `line 1: not JSON`. Een tekstlog met een regel als `{'a': 1}` wordt als geheel geweigerd, met opzet: een regel die een object opent en geen JSON is, mag geen aanvraag ongezien laten vallen.

Elk bestand dat je geeft wordt in het geheugen gelezen. Alle invoerbestanden van één run samen (bestanden, mappen en standaardinvoer) mogen niet meer dan 268435456 bytes zijn, geteld voordat elk bestand wordt gelezen (`input is larger than 268435456 bytes in total`, exit 2), en één gzip-bestand mag hoogstens 1000000 leden hebben (`more than 1000000 gzip members`, exit 2). Een run meldt hoogstens 100000 rijen, één per functie, architectuur en geheugengrootte; meer is `more than 100000 function and memory rows`, exit 2, ook met `--json`. De `--prices`- en `--functions`-bestanden tellen daar niet in mee.

Een JSON-document, een `--prices`-bestand en een `--functions`-bestand mogen samen hoogstens 4000000 JSON-waarden bevatten: een scalar en een objectsleutel tellen 1, een array of een object telt 4. De waarden worden geteld voordat het document wordt opgebouwd, ook voor de tekst in een gzip-bestand, en een document boven het budget is `more than 4000000 JSON values`, afsluitcode 2. De tekstgrootte zegt weinig over het geheugen van een JSON-document (een scalar heeft ongeveer 100 bytes nodig, een kleine array ongeveer 300), dus het budget is wat het begrenst. Gemeten met een releasebuild op Linux x86_64, piekgeheugen van documenten op het budget die daarna als geen logevenement werden geweigerd: 3990000 scalars `[0,0,...]` (7999981 bytes) 424108 KB in 0,6 s; 1998999 objectsleutels (24875884 bytes) 623372 KB in 2,3 s; 280000 CloudWatch-gebeurtenissen met elk vier velden (19040001 bytes) 356556 KB in 0,7 s. Een document op het budget heeft ruim onder 1 GB en een paar seconden nodig (het budget was 20000000 waarden, tot ongeveer 3 GB en 17 s). Boven het budget werden een `[0,0,...]` van 100000001 bytes, een `[[0],...]` van 100000001 bytes en een gzip-bestand van 657179 bytes met 170 MB aan geneste arrays in minder dan 1 s en met minder dan 200 MB geweigerd (gemeten bij het oude budget; de telpas stopt nu eerder). Een CloudWatch-export met vier velden per gebeurtenis haalt het budget bij ongeveer 330000 gebeurtenissen. Een JSON-regel wordt begrensd door de regellimiet van 1048576 bytes, niet door dit budget.

Andere gemeten kosten binnen de bestandslimiet (releasebuild): een gzip-bestand met 13000000 lege leden duurde 39,3 s en 255808 KB vóór de ledengrens (nu geweigerd); een bestand met 100000 gebeurtenissen die elk een andere functie noemen duurde 1,0 s en 119244 KB (gemeten voordat de rijgrens bestond, op de grens; een bestand met meer functies wordt nu geweigerd, zie hierboven). Elke rij (functie, architectuur en geheugengrootte) kost ongeveer 10 microseconden, en een bestand met 10113 geheugengroottes van één functie (1465123 bytes) kost 0,14 s. Dit alles groeit lineair met de invoer.

Een GB-seconde, een duurbedrag, een aanvraagbedrag, een adviesbedrag en een totaal worden alleen opgeslagen als ze al een exact 96-bits decimaal getal zijn: een coëfficiënt van hoogstens `79228162514264337593543950335` en hoogstens 28 cijfers achter de komma. Vermenigvuldigen en delen houden elk cijfer. Een waarde die nog geen zo'n decimaal getal is, is `price overflow`, afsluitcode 2. Er wordt niets afgerond. Twee aanvragen tegen `79228162514264337593543950335` USD per miljoen passen: het bedrag is `158456325028528675187087.90067`. Eén aanvraag is `79228162514264337593543.950335`. Eén GB-seconde tegen `10^-28` USD is `0.0000000000000000000000000001`. 10240 MB gedurende 18446744073709551615 ms tegen `1000000000000` USD per GB-seconde past niet.

Deze passen niet. Elk eindigt met 2 en drukt niets op standaarduitvoer. 1000 MB gedurende 1 ms tegen `1000000000000000000000001` USD per GB-seconde is precies `976562500000000000000.0009765625`. 128 MB gedurende 1000 ms tegen `7922816251426433759354395033` is precies `990352031428304219919299379.125`. 512 MB gedurende 1000 ms tegen `79228162514264337593543950333` is precies `39614081257132168796771975166.5`. 1023 MB gedurende 18446744073709551615 ms is geen exact decimaal getal. Eén rij van `39614081257132168796771975168` plus één rij van `0.50` is geen exact decimaal getal. Tegen `1237940039285380274899124224.5` USD per GB-seconde past de rij van 1024 MB gedurende 1000 ms, en het adviesbedrag voor een verhoging van 1024 MB naar 1229 MB past niet. Hetzelfde tarief op 1024 MB houden past wel. De tabel toont het opgeslagen decimaal getal en rondt het niet opnieuw af. De langste aanroep van Lambda is 900000 ms.

`--headroom` neemt een geheel getal van 0 tot 100. `+5` en `05` worden als 5 gelezen.

`--quiet` drukt alleen het resultaat en fouten: de waarschuwingen op standaardfout (de JSON-waarschuwingen en de waarschuwing over de prijsleeftijd) worden niet gedrukt, en het rapport op standaarduitvoer is hetzelfde. `--verbose` voegt voortgang op standaardfout toe: één regel `lambda-bill: reading <bestand>` per argument en een afsluitende regel `lambda-bill: done: <n> functions, <n> requests`. `--quiet` samen met `--verbose` is een gebruiksfout met afsluitcode 3 en drukt niets anders.

`--output BESTAND` schrijft het rapport (tekst, `--json` of `--md`) naar het bestand in plaats van naar standaarduitvoer en drukt niets op standaarduitvoer. Het rapport gaat naar een tijdelijk bestand in dezelfde map en wordt dan over het doel hernoemd, dus een mislukte of onderbroken run laat een bestaand bestand zoals het was en geen half geschreven rapport. Een bestaand gewoon bestand wordt vervangen (`--verbose` meldt dat met een regel `replacing`). Een symlink, een map of een apparaat op dat pad wordt geweigerd en nooit gevolgd. Een ontbrekende map wordt niet aangemaakt. Elke schrijffout geeft afsluitcode 2 met één regel `cannot write`.

SIGINT (Ctrl-C) eindigt met code 130 en drukt `lambda-bill: interrupted by SIGINT` op standaardfout. Er wordt niets op standaarduitvoer gedrukt, dus een lege uitvoer is nooit een succes. Een SIGINT die aankomt terwijl het rapport wordt geschreven, laat die schrijfactie afmaken en eindigt dan met 130. De handler wordt alleen op Unix geïnstalleerd; op Windows blijft de standaard.

Prijsleeftijd: als de datum van het prijsbestand (het ingebouwde of `--prices`) meer dan 180 dagen vóór `--as-of DATUM` (`JJJJ-MM-DD`; vandaag in UTC als die niet is gegeven) ligt, gaat een regel `warning:` met die datum naar standaardfout. De afsluitcode blijft 0. Een foute `--as-of`-datum eindigt met 2.

Een som die niet in een decimaal getal past is de fout `price overflow`, geen panic. Als standaarduitvoer bij de start van het programma gesloten was (`>&-`), alleen-lezen is geopend (`1<bestand`), of het schrijven mislukt (bijvoorbeeld `/dev/full` of een gesloten pipe), is de afsluitcode 2 en zegt standaardfout dat. Dit geldt ook voor `--help` en `--version`. `>/dev/null` en `1<>/dev/null` zijn open en eindigen met 0. Elke andere invoer- of gebruiksfout eindigt met 2.

De ingebouwde tarieven zijn de cijfers van de eerste tier voor US East (N. Virginia) op de prijspagina van AWS Lambda, gelezen op 2026-11-09. Andere regio's en tiers zijn niet ingebouwd. Zet ze in een `--prices`-bestand als je een offerte hebt. Het programma heeft geen AWS SDK en opent geen netwerkverbinding.

## Prijzen en verantwoordelijkheid

De ingebouwde prijzen zijn een momentopname: de on-demandlijst van AWS Lambda voor de eerste tier van US East (N. Virginia), bron `https://aws.amazon.com/lambda/pricing/`, peildatum 2026-11-09 (`prices/lambda-ondemand.json`). Elk rapport toont die bron en datum, en er gaat een waarschuwing naar standaardfout als de datum meer dan 180 dagen vóór `--as-of` (of vandaag) ligt.

Controleer zelf de actuele prijzen voordat je op een getal vertrouwt. Jij bent zelf verantwoordelijk voor beslissingen op basis van de uitvoer van dit programma; het is een schatting uit lijstprijzen, niet je factuur. Geef met `--prices BESTAND` je eigen prijsbestand mee.

## Onderhoud

Meld bugs als GitHub Issue in deze repository. Na een release staat de tool in sluimermodus: een nieuwe versie komt alleen bij een gemelde bug, een aanvraag of een beveiligingsprobleem, en beveiliging gaat altijd voor.

## Ondersteuning

Bugs en vragen: GitHub Issues van deze repository. Een beveiligingsprobleem gaat via de privéroute in [SECURITY.md](SECURITY.md). Bouwen en testen staat in [CONTRIBUTING.md](CONTRIBUTING.md). De uitgevoerde controles staan in [QUALITY.nl.md](QUALITY.nl.md).

## Licentie

Beschikbaar onder een van beide licenties:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT-licentie ([LICENSE-MIT](LICENSE-MIT))

naar jouw keuze.

De licenties van de externe crates in het programma staan in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).

## Contact

- E-mail: pep_ai@icloud.com
- Web: https://www.pepai.nl
- X: https://x.com/pepai_nl
- Ko-fi: https://ko-fi.com/pepai
