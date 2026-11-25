# Kwaliteit

De meting hieronder geldt voor deze commit.

Suite: 273 tests: 273 geslaagd, 0 mislukt, 0 genegeerd.

Reparatie van 2026-11-23, bevinding B1 van de P-review van 73df4b1: `looks_like_json_array` in `src/parse.rs` sloeg na de openende haak alleen spatie en tab over, dus een afgekapte array met een CR na de haak (`[`, CR, `{"message":...,"logGroup":"/aws/lambda/other","architecture":"arm64"}`, zonder sluitende haak) werd als tekst gelezen, geteld en gefactureerd op `--function f` op x86_64 met exit 0. De codecommit 282f8e2 slaat spatie, tab, CR en LF over. Een gewone haakregel (`[INFO]`, `[ERROR]`, `[WARNING]`, `[main]`, `[JJJJ-MM-DD`) blijft tekst. Dezelfde bestanden voor (tip 73df4b1) en na (282f8e2), releasebuild, `lambda-bill --function f <bestand>`, uitvoer gefilterd op de foutregel, de tabelrijen en de exitcode. `base.json` is de afgekapte array zonder CR, `c.json`, `spcr.json` en `tabcr.json` hebben CR, spatie+CR en tab+CR na de haak, `valid.json` is de gesloten array met CR, `text.log` heeft `[INFO]`-, `[main]`- en `[2024-01-01]`-regels rond een REPORT.

Voor, op 73df4b1:

```
== base.json
lambda-bill: base.json: line 1: not JSON
exit 2
== c.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
advice f x86_64 512 MB: downsize to 128 MB; duration USD 0.000000833335 -> 0.00000020833375 if billed duration stays the same
exit 0
== spcr.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
advice f x86_64 512 MB: downsize to 128 MB; duration USD 0.000000833335 -> 0.00000020833375 if billed duration stays the same
exit 0
== tabcr.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
advice f x86_64 512 MB: downsize to 128 MB; duration USD 0.000000833335 -> 0.00000020833375 if billed duration stays the same
exit 0
== valid.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds   duration_usd  request_usd      total_usd  advice
other     arm64               512           40         1            0        0.05  0.00000066667    0.0000002  0.00000086667  downsize to 128 MB
total                                                  1            0        0.05  0.00000066667    0.0000002  0.00000086667
advice other arm64 512 MB: downsize to 128 MB; duration USD 0.00000066667 -> 0.0000001666675 if billed duration stays the same
exit 0
== text.log
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
exit 0
```

Na, op 282f8e2:

```
== base.json
lambda-bill: base.json: line 1: not JSON
exit 2
== c.json
lambda-bill: c.json: line 1: not JSON
exit 2
== spcr.json
lambda-bill: spcr.json: line 1: not JSON
exit 2
== tabcr.json
lambda-bill: tabcr.json: line 1: not JSON
exit 2
== valid.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds   duration_usd  request_usd      total_usd  advice
other     arm64               512           40         1            0        0.05  0.00000066667    0.0000002  0.00000086667  downsize to 128 MB
total                                                  1            0        0.05  0.00000066667    0.0000002  0.00000086667
advice other arm64 512 MB: downsize to 128 MB; duration USD 0.00000066667 -> 0.0000001666675 if billed duration stays the same
exit 0
== text.log
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
exit 0
```

Gewenst: `c.json`, `spcr.json` en `tabcr.json` geven exit 2 met `not JSON` en tellen niets (ze waren exit 0, 1 request, functie f, x86_64); `base.json` blijft exit 2; `valid.json` en `text.log` blijven exit 0 zoals eerst. Suite op 282f8e2: 273 tests, 273 geslaagd. De bestaande test `a_bracket_prefix_that_is_not_json_stays_text_and_a_cut_array_is_not_json` heeft er nu ook zeven afgekapte arrays met CR bij; met de oude `src/parse.rs` faalde hij (`assertion left == right failed ... left: 0, right: 2`), met de reparatie slaagt hij. Clippy `-D warnings` en `cargo fmt --check` zijn schoon. Nog open: dezelfde review noemt andere eerste tekens na de haak (`\f`, `\v`, vaste spatie, zero-width spatie, NUL, BOM) die deze reparatie niet raakt; de grens `[1,2] x` blijft zoals eerst. Dit is een reparatie, geen eindoordeel.

Reparatie van 2026-11-23, bevinding uit de review van 0a1ebb8: sinds 080dab6 was elke regel die met `[` begint en geen JSON en geen `[JJJJ-MM-DD` is `not JSON` (exit 2), dus één gewone `[INFO]`-regel stopte de hele map, terwijl de CHANGELOG zegt dat zo'n regel tekst is. Codecommit e710e44 leest een `[`-regel die geen JSON is als tekst, tenzij hij opent als een JSON-array (`{`, `[`, `]`, `"`, `true`/`false`/`null`, alleen getallen, of niets na het haakje). Dezelfde vier bestanden, `lambda-bill --json <bestand>`, uitvoer gefilterd op de foutregel en de regel `"requests"`, releasebuild, letterlijke uitvoer.

Vóór, op 0a1ebb8:

```
== info.log
   | # function: f
   | [INFO] start
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [ERROR] x
   | [WARNING] y
   | [main] z
lambda-bill: info.log: line 2: not JSON
exit 2
== cut.log
   | # function: f
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [{"message":"REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 
lambda-bill: cut.log: line 3: not JSON
exit 2
== nobracket.log
   | # function: f
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [{"message":"x"}
lambda-bill: nobracket.log: line 3: not JSON
exit 2
== date.log
   | # function: f
   | [2024-01-01] REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 
    "requests": 1,
exit 0
```

Na, op e710e44:

```
== info.log
   | # function: f
   | [INFO] start
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [ERROR] x
   | [WARNING] y
   | [main] z
    "requests": 1,
exit 0
== cut.log
   | # function: f
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [{"message":"REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 
lambda-bill: cut.log: line 3: not JSON
exit 2
== nobracket.log
   | # function: f
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [{"message":"x"}
lambda-bill: nobracket.log: line 3: not JSON
exit 2
== date.log
   | # function: f
   | [2024-01-01] REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 
    "requests": 1,
exit 0
```

Gewenst: `info.log` tekst en exit 0 (was exit 2), `cut.log` en `nobracket.log` blijven exit 2 en tellen niets, `date.log` blijft tekst en exit 0. Suite op e710e44: 273 tests, 273 geslaagd; de nieuwe test is `a_bracket_prefix_that_is_not_json_stays_text_and_a_cut_array_is_not_json`, en `a_plain_line_with_a_glued_marker_is_ignored_and_a_json_line_with_one_is_an_error` verwacht nu `not a Lambda REPORT line` voor `[ts] x<marker>` (nog exit 2, lege standaarduitvoer). Niet gerepareerd, nog open: veel kleine gzip-bestanden, en `--md` met bestandsnamen. Grens: een tekstregel als `[1,2] x` of `[true] x` opent als een array en blijft `not JSON`.

Deze commit wijzigt alleen documentatie. Bij elk getal hieronder staat op welke commit het gemeten is; een rij die niet is herhaald zegt dat. De Engelse [QUALITY.md](QUALITY.md) is leidend voor alle details en metingen; dit bestand geeft dezelfde feiten in het Nederlands, met de lange meetparagrafen samengevat.

Registratie van 2026-11-23, codecommit e35a324 (`src/`, `tests/` en het manifest zijn gelijk aan die van 40aa20a en fe0a063; de commits na e35a324 wijzigen alleen documenten). Linux x86_64, rustc 1.99.0 en cargo 1.99.0 (stable). Voor deze registratie gemeten op e35a324: debug `cargo test --locked --offline`: 271 geslaagd, 0 mislukt, 0 genegeerd, waarvan `cargo test --locked --offline --lib` 66 geslaagd printte; `cargo fmt --check` en `cargo clippy --locked --offline --all-targets -- -D warnings`: afsluitcode 0. Gemeten op fe0a063 door de eindreview: `cargo fmt --check`, clippy, de debug- en de release-testrun (telkens 271 geslaagd, 0 mislukt), `cargo package` en `cargo deny`; `cargo +1.85` compileerde de tests alleen (`--no-run`), de 1.85-tests: not run. Elke andere rij hieronder is de registratie van 2026-11-20 op commit 2d51303 (toen 269 tests) en is niet herhaald. CI voor deze commit: not run. Elke overgeslagen controle staat één keer onder Niet uitgevoerd hieronder. Geen tag, geen publicatie, geen push.

Rood voor groen, 2026-11-23, bevinding B-1 van de eindreview: een regel die met `[` begint, geen `[YYYY-MM-DD`-tijdstempel is en geen JSON is, ging naar de tekstparser, zodat een afgekapte array zijn `logGroup` en `architecture` verloor en met exit 0 op `--function` en x86_64 werd gerekend. De nieuwe test `a_broken_json_array_line_is_not_json_and_a_valid_one_keeps_its_envelope` draaide eerst op de code van 61a50a1 en faalde: `thread 'a_broken_json_array_line_is_not_json_and_a_valid_one_keeps_its_envelope' panicked at tests/cli.rs:2357:9: assertion `left == right` failed: cut`, `left: 0`, `right: 2`, `test result: FAILED. 0 passed; 1 failed`. Met de fix in `src/parse.rs` (commit 080dab6) slaagt hij, en `a_big_text_log_that_starts_with_a_bracket_is_not_counted_as_json` blijft groen. Debug `cargo test --locked --offline --no-fail-fast` op 080dab6: 272 tests, 271 geslaagd, 1 mislukt: `the_documents_state_the_real_number_of_tests`, omdat README.md en README.nl.md nog 271 zeggen; de testrijen in de tabellen hieronder zijn de getallen van e35a324. Twee bestaande tests (`a_plain_line_with_a_glued_marker_is_ignored_and_a_json_line_with_one_is_an_error`, `uncounted_report_marker_in_json_or_bracket_line_exits_2`) verwachten nu `not JSON` voor een haakjesregel die geen tijdstempel is; de afsluitcode is 2 en de standaarduitvoer is leeg, zoals eerder.

Rood voor groen, 2026-11-23, op een wegwerpkopie buiten deze repository: `MAX_JSON_UNITS` in `src/budget.rs` ging van 4_000_000 naar 400_000_000. `a_json_document_with_too_many_values_is_refused_before_it_is_built` faalde toen (`tests/cli.rs:2260:5`, `lambda-bill: /big.json: event 1: not a log event`, `test result: FAILED. 0 passed; 1 failed`). Na terugdraaien slaagde hij weer. De kopie staat niet in de repository.

Tests zijn onafhankelijk van de systeemklok: elke test die het programma draait geeft een vaste `--as-of 2026-11-20` mee (of een eigen datum). Bewijs, vóór de fix: in een kopie van de map is `src/money.rs` (`today_utc`) zo aangepast dat de klok 400 dagen vooruit loopt, en `cargo test --no-fail-fast` faalde toen met minstens 27 tests in `tests/cli.rs` (`assertion failed: stderr.is_empty()` en andere). Met de fix slaagt dezelfde kopie voor alle tests (toen 258).

## Opdrachten

De kolom Gemeten op van deze tabel zegt op welke commit elke rij het laatst draaide: e35a324 (gedraaid voor deze registratie, met `-j 2` en `--test-threads 2`), fe0a063 (de eindreview) of 2d51303 (de registratie van 2026-11-20, niet herhaald; zie Niet uitgevoerd).

| Opdracht | Afsluitcode | Gemeten op |
| --- | ---: | --- |
| `cargo fmt --check` | 0 | e35a324 and fe0a063 (final review) |
| `cargo clippy --locked --offline --all-targets -- -D warnings` (stable) | 0 | e35a324 and fe0a063 (final review) |
| `cargo +1.85 clippy --locked --offline --all-targets -- -D warnings` | 0 | 2d51303 |
| `cargo check --locked --offline --tests --target x86_64-pc-windows-gnu` | 0 | 2d51303 |
| `cargo check --locked --offline --tests --target x86_64-apple-darwin` | 0 | 2d51303 |
| `cargo check --locked --offline --tests --target x86_64-unknown-linux-musl` | 0 | 2d51303 |
| `cargo clippy --locked --offline --all-targets --target x86_64-pc-windows-gnu -- -D warnings` | 0 | 2d51303 |
| `cargo clippy --locked --offline --all-targets --target x86_64-apple-darwin -- -D warnings` | 0 | 2d51303 |
| `cargo clippy --locked --offline --all-targets --target x86_64-unknown-linux-musl -- -D warnings` | 0 | 2d51303 |
| `cargo test --locked --offline` (debug) | 0 | e35a324 and fe0a063 (final review) |
| `cargo test --locked --offline --lib` (66 geslaagd) | 0 | e35a324 |
| `cargo test --locked --offline --release` | 0 | fe0a063 (final review) |
| `cargo +1.85 test --locked --offline` (debug) | 0 | 2d51303 (op fe0a063 zijn de tests alleen gecompileerd) |
| `cargo +1.85 test --locked --offline --no-run` | 0 | fe0a063 (final review) |
| `cargo package --locked --offline --allow-dirty --no-verify --list` | 0 | fe0a063 (final review) |
| `cargo package --locked --offline --allow-dirty --no-verify`, then `cargo test --offline` in the unpacked crate | 0 | 2d51303 |
| `bash scripts/check-glibc.sh target/release/lambda-bill 2.34` | 0 | 2d51303 |
| `cargo deny --locked check licenses` | 0 | fe0a063 (final review) |
| `cargo audit --no-fetch` | 0 | 2d51303 |

De rijen voor Windows, macOS en musl compileren de code en de tests en draaien clippy. Op die doelen is niets gedraaid. `cargo deny` meldde `licenses ok`. `cargo audit` las 1527 advisories (database van de machine, niet ververst) voor 76 crates en meldde er geen. `check-glibc.sh` meldde `highest GLIBC symbol: 2.34 (allowed: 2.34)` voor de releasebinary van deze machine.

`cargo package --list` heeft 50 regels: `.cargo_vcs_info.json`, `.gitignore`, `CHANGELOG.md`, `CONTRIBUTING.md`, `Cargo.lock`, `Cargo.toml`, `Cargo.toml.orig`, beide licenties, `QUALITY.md`, `QUALITY.nl.md`, `README.md`, `README.nl.md`, `SECURITY.md`, `about.hbs`, `about.toml`, `build.rs`, `deny.toml`, `examples/*` (tien bestanden), `man/lambda-bill.1`, `prices/lambda-ondemand.json`, `src/*.rs` (dertien bestanden) en `tests/*.rs` (zeven bestanden). `.github/`, `.gitattributes` en `scripts/` zijn uitgesloten (het manifest noemt ze in `exclude`).

## Testresultaat per doel

Debug (`cargo test --locked --offline`) op e35a324 gaf de aantallen hieronder; de lib-rij is wat `cargo test --locked --offline --lib` printte (66). De releasekolom is het aantal van de eindreview op fe0a063 (`--release`, dezelfde bronnen), voor deze registratie niet herhaald. De tests van `cargo +1.85`: not run op deze boom.

| Doel | Debug geslaagd | Debug mislukt | Debug genegeerd | Release geslaagd | Release mislukt | Release genegeerd |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| unittests `src/lib.rs` | 66 | 0 | 0 | 66 | 0 | 0 |
| unittests `src/main.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `tests/claims.rs` | 39 | 0 | 0 | 39 | 0 | 0 |
| `tests/cli.rs` | 139 | 0 | 0 | 139 | 0 | 0 |
| `tests/documented_shapes.rs` | 9 | 0 | 0 | 9 | 0 | 0 |
| `tests/docs_clean.rs` | 3 | 0 | 0 | 3 | 0 | 0 |
| `tests/handbook_help.rs` | 1 | 0 | 0 | 1 | 0 | 0 |
| `tests/quality.rs` | 4 | 0 | 0 | 4 | 0 | 0 |
| `tests/readme.rs` | 11 | 0 | 0 | 11 | 0 | 0 |
| doc-tests | 0 | 0 | 0 | 0 | 0 | 0 |
| total | 273 | 0 | 0 | 273 | 0 | 0 |

## Standaarduitvoer gesloten, alleen-lezen of vol

`src/stdout_probe.rs` herkent een standaarduitvoer die bij de start van het programma gesloten of alleen-lezen was. Rust geeft een gesloten bestandsbeschrijver 1 vóór `main` terug als `/dev/null` (lezen en schrijven), en negeert de fout van een schrijfactie naar een beschrijver die niet voor schrijven is geopend. Daarna lijkt `>&-` dus op `1<>/dev/null` en `1<bestand` op een werkende uitvoer. Op Linux draait een constructor in `.init_array` eerder en legt de toestand van beschrijver 1 vast. `src/main.rs` weigert dan te schrijven en eindigt met 2. Op andere systemen zegt de probe altijd "schrijfbaar". Zijn `unsafe`-code is twee alleen-lezen aanroepen `fcntl(1, ...)` (`F_GETFD`, `F_GETFL`). De enige andere `unsafe`-code is de SIGINT-handler in `src/interrupt.rs` (zie SECURITY.md).

Gemeten op 2026-11-19 met `examples/sample-report.log`. De debug- en de releasebinary gaven in elke rij dezelfde afsluitcode en melding.

| Opdracht | Afsluitcode | Standaardfout |
| --- | ---: | --- |
| `lambda-bill examples/sample-report.log >&-` | 2 | `lambda-bill: could not write output` |
| `lambda-bill examples/sample-report.log >/dev/full` | 2 | `lambda-bill: could not write output` |
| `lambda-bill examples/sample-report.log >/dev/null` | 0 | none |
| `bash -c 'lambda-bill examples/sample-report.log 1<>/dev/null'` | 0 | none |
| `lambda-bill examples/sample-report.log 1<examples/sample-report.log` | 2 | `lambda-bill: could not write output` |
| `lambda-bill examples/sample-report.log 1<.` | 2 | `lambda-bill: could not write output` |
| `lambda-bill --json examples/sample-report.log >&-` | 2 | `lambda-bill: could not write output` |
| `python3 -I -c '... subprocess.run([...], stdout=subprocess.DEVNULL)'` | 0 | none |
| `lambda-bill examples/sample-report.log \| true` (10 runs) | 2 each | `lambda-bill: could not write output` |
| `lambda-bill --help >&-` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --version >&-` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --help >/dev/full` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --version >/dev/full` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --help 1<examples/sample-report.log` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --help >/dev/null` | 0 | none |
| `lambda-bill --version 1<>/dev/null` | 0 | none |
| `lambda-bill --help \| true` (10 runs) | 2 each | `lambda-bill: stdout is not writable` |
| `lambda-bill --version \| true` (10 runs) | 2 each | `lambda-bill: stdout is not writable` |

`python3` met `stdout=subprocess.DEVNULL` gaf afsluitcode 0 en 0 bytes standaardfout voor het rapport, `--help` en `--version`. Dezelfde rijen met `/proc` vervangen door een lege `tmpfs` (`unshare -Urm`) gaven dezelfde afsluitcodes. Geen enkele run gaf een panic. Of een gesloten pipe het schrijven laat falen hangt af van het moment waarop de lezer stopt. De tests `closed_stdout_exits_2_without_panic`, `dev_full_stdout_exits_2`, `read_only_stdout_exits_2`, `help_and_version_to_unwritable_stdout_exit_2`, `dev_null_stdout_exits_0` en `read_write_dev_null_is_not_a_closed_stdout` (`tests/cli.rs`, alleen Linux) dekken de rijen `>&-`, `/dev/full`, `1<bestand`, `/dev/null`, `1<>/dev/null`, `--help` en `--version`. De rijen met `DEVNULL`, zonder `/proc` en met `| true` hebben geen test.

## Gebruiksfouten

Een gebruiksfout die clap drukt behoudt zijn regeleinden en eindigt met een regeleinde. Gemeten met de releasebinary: `lambda-bill --headroom $'1\x1b[31m\nFAKE: line' examples/sample-report.log` en `lambda-bill $'--x\x1b[31m\nFAKE: line' examples/sample-report.log` eindigen allebei met 2 en drukken het argument als `\u{1b}[31m\u{a}FAKE: line` binnen de eigen regels van clap, zonder regel die met `FAKE` begint en zonder ESC-byte. Een argument dat geen geldige UTF-8 is gaf de eigen melding van clap, die het niet herhaalt. Tests: `clap_errors_keep_their_line_breaks`, `clap_error_with_escape_and_newline_in_an_argument_has_no_fake_line`, `clap_error_escapes_cr_bidi_and_invisible_characters_on_one_line`.

## Rood vóór, groen erna

Gedraaid op 2026-11-19 tegen commit dc217f5 (de tests van deze toestand, de bronnen van dc217f5), daarna tegen deze toestand. De tabel staat in [QUALITY.md](QUALITY.md).

| Test | dc217f5 | Deze toestand |
| --- | --- | --- |
| `an_escaped_marker_in_a_field_that_is_not_read_is_an_error` | fails: `REPORT\u0020RequestId:` in `@message` gives exit 0 and one request | passes |
| `a_fifo_as_prices_or_functions_file_is_not_a_file_and_does_not_hang` | fails: the run is still blocked after 5 s | passes |
| `two_different_log_group_names_in_one_object_are_named_in_the_error` | fails: the message says the log group is not `/aws/lambda/<name>` | passes |
| `a_json_document_with_too_many_values_is_refused_before_it_is_built` | fails: `[[0],...]` of 16400005 bytes is built in full and ends as `not a log event`, no budget error | passes |
| `a_small_gzip_file_cannot_expand_into_too_many_values` | fails: the same text inside a gzip file of under 100000 bytes is built in full | passes |
| `a_prices_or_functions_file_with_too_many_values_is_refused` | fails: a `--prices` file with 4100000 extra arrays is accepted, exit 0 | passes |

De andere tests die in die toestand zijn toegevoegd leggen gedrag vast dat al bestond en nu is gedocumenteerd, dus ze slagen ook op dc217f5. Op commit a7d2dd6, vóór de wijziging naar exacte decimalen, eindigden `a_duration_that_needs_more_than_28_digits_exits_2` en vier verwante tests met 0 en drukten een afgerond bedrag; nu eindigen ze met 2. De details staan in [QUALITY.md](QUALITY.md).

## Niet uitgevoerd

- Een fuzz-campagne.
- Een mutatierun.
- `cargo audit` met een ververste advisory-database (er is geen netwerk gebruikt).
- De rijen van de opdrachtentabel waar Gemeten op 2d51303 zegt (de 1.85-clippy- en testrijen, de rijen voor Windows, macOS en musl, de test van het uitgepakte pakket, `check-glibc.sh` en `cargo audit`): op deze commit not run. De tests van `cargo +1.85`: not run, op fe0a063 alleen gecompileerd.
- GitHub Actions: noch `.github/workflows/ci.yml` noch `.github/workflows/release.yml` is gedraaid, wegens het betaalprobleem van het account. Dus de job `glibc` (Ubuntu 22.04), de tijdslimieten, de releasebuild op vijf doelen, de rooktest van het archief en de CI-badge in de README hebben geen resultaat. Het glibc-cijfer hierboven komt van deze machine, en de releaseworkflow is alleen als YAML gecontroleerd.
- `directory_at_64_levels_is_accepted` bouwt een pad van enkele honderden tekens; op Windows is het niet gedraaid.
- Elke test op Windows, macOS of musl. Alleen de compileer- en clippy-rijen hierboven draaiden daar. De tests met gesloten standaarduitvoer en die met een ESC-byte in een bestandsnaam zijn alleen Linux, de symlinktests alleen Unix, en de probe meldt op andere systemen "schrijfbaar".
- `git push`, `git tag` en `cargo publish`: er is niets gepusht, getagd of gepubliceerd. De commits zijn lokaal, dus de standaardtak op GitHub toont deze hash nog niet.
- De opdrachten van de snelstart in de README met de geïnstalleerde binary (`cargo install`): de test draait dezelfde bytes door de testbinary.
- `cargo test` met `--ignored`: de suite heeft geen genegeerde test.

## Bekende grenzen

- Deze pagina legt controles vast. Het is geen garantie en geen fuzzrapport.
- Een gesloten of alleen-lezen standaarduitvoer wordt alleen op Linux herkend. Elders wordt het niet gezien en beslist de schrijfactie zelf.
- `lambda-bill ... | true` eindigt met 2 als de lezer de pipe al heeft gesloten bij het schrijven. Een lezer die later sluit kan afsluitcode 0 zien. Geen test legt dit vast.
- Geheugen: elk bestand wordt in het geheugen gelezen, en de limiet van 268435456 bytes geldt per bestand, niet voor de hele run. Een JSON-document, een `--prices`-bestand en een `--functions`-bestand worden geteld voordat ze worden opgebouwd en boven 4000000 waarden geweigerd. Gemeten met de releasebinary op dit budget (4000000) was het piekgeheugen van documenten op het budget 356556 tot 623372 KB (0,6 tot 2,3 s); bij het oude budget van 20000000 waarden was dat tot 2846368 KB en 17,2 s. Boven het budget duurde de weigering 0,5 tot 0,8 s en minder dan 200 MB. Geen test meet het geheugen zelf; de tests controleren de weigering. De volledige cijfers staan in [QUALITY.md](QUALITY.md).
- Een gzip-bestand van veel lege leden is traag binnen de bestandslimiet: 13000000 lege leden en één echt lid (260000109 bytes) duurden 39,3 s en 255808 KB met afsluitcode 0, omdat elk lid een nieuwe decoder start. Het groeit lineair. Productbesluit, open punt: een maximum aantal leden of een tijdslimiet. Geen test (`gap`).
- Een run meldt hoogstens 100000 rijen (één per functie, architectuur en geheugengrootte). Meer is `more than 100000 function and memory rows`, afsluitcode 2, ook met `--json`; de test `a_run_with_too_many_functions_is_refused` legt dit vast. De grens vervangt het eerdere open punt over bestanden met heel veel functies; voor deze registratie is geen tijd gemeten.
- Een JSON-regel wordt begrensd door de regellimiet (1048576 bytes), niet door het waardebudget. Zo'n regel wordt één voor één opgebouwd en vrijgegeven.
- De crate is niet gepubliceerd en heeft geen tag, dus er bestaat geen releasearchief (de releaseworkflow: zie Niet uitgevoerd).
- Het JSON-logformaat van Lambda (records `type: platform.report`) wordt niet gelezen. Zo'n record wordt zonder bericht overgeslagen en zijn aanvraag telt niet mee (`platform_report_records_are_not_read`).
- Een opgemaakt JSON-object dat geen CloudWatch-gebeurtenis is en geen `@message` of `Message` heeft, wordt geweigerd als `line 1: not JSON`. Een tekstlog met een regel als `{'a': 1}` wordt als geheel geweigerd, met opzet.
- Een GB-seconde, een duurbedrag, een aanvraagbedrag, een adviesbedrag en een totaal worden alleen opgeslagen als ze al een exact 96-bits decimaal getal zijn (coëfficiënt hoogstens 79228162514264337593543950335, hoogstens 28 cijfers achter de komma). Vermenigvuldigen en delen houden elk cijfer. Al het andere is `price overflow`, afsluitcode 2. Er wordt niets afgerond. De voorbeelden staan in [QUALITY.md](QUALITY.md) en in de README (Beperkingen).
- Een pad dat letterlijk `bad\xff\xfe.log` heet, wordt op dezelfde manier getoond als een pad met de bytes ff en fe.
- Het totaal is de exacte som van de rijbedragen. Een som die geen exact decimaal getal is, is `price overflow`. De tabel toont het opgeslagen decimaal getal (bijvoorbeeld `0.00000165833585`) en rondt het niet opnieuw af.
- "Geen netwerkverbinding" wordt gecontroleerd via de afhankelijkhedenlijst (`cargo tree -e normal`), een zoektocht in `src/` naar socket- en procesaanroepen (de enige bestandsschrijfacties zijn het tijdelijke bestand, de hernoeming en het opruimen van `--output` in `write_output_file`, `src/main.rs`; `build.rs` start bij het bouwen `git`), en `claim_c17_no_aws_sdk_and_no_network`. Geen test bewaakt systeemaanroepen.
- Het aantal tests in `README.md` en in dit bestand wordt vergeleken met het aantal testattributen in `src/` en `tests/` (`tests/quality.rs`), niet met de uitvoer van `cargo test`.
- Een GB-seconde-quotiënt dat geen exact decimaal getal is, is `price overflow`, afsluitcode 2. Gemeten: 18446744073709551615 ms op 1023 MB (`gb_seconds_quotient_beyond_28_digits_is_an_error`). Het werd vroeger afgerond opgeslagen. De langste aanroep van Lambda is 900000 ms.
- Een product, een som of een adviesbedrag dat geen exact decimaal getal is, is `price overflow`, afsluitcode 2; de gemeten gevallen staan in [QUALITY.md](QUALITY.md).
- Billed Duration wordt gelezen met hoogstens 20 tekens en Memory Size en Max Memory Used met hoogstens 10 tekens; de grens is de lengte, niet de waarde (`billed_duration_and_memory_fields_are_limited_by_their_text_length`).
- Tekst vóór een JSON-regel (een byte-order-mark, een komma, `data: `, een tijdstempel) maakt van de regel tekst, en de markering erin telt niet mee (`text_before_a_json_line_makes_it_text_and_its_marker_does_not_count`).
- Een `# function:`-kop na de eerste REPORT geeft nog steeds het hele bestand zijn naam; twee verschillende koppen zijn een fout.
- Een RequestId dat twee keer voorkomt voor één functie met andere geheugen- of duurwaarden telt één keer, de eerste kopie wint, en de waarschuwing telt alleen de duplicaten.
- Een bestand van 268435456 lege regels duurde 3,3 s en 526208 KB met de releasebinary; de debugbuild is veel trager (zie [QUALITY.md](QUALITY.md)).
- De bestanden `--prices` en `--functions` moeten gewone bestanden zijn; een FIFO of `<(opdracht)` is `not a file` en hangt niet.
- Een veld dat na de vier vaste velden van een REPORT-regel wordt herhaald, is trailing tekst en wordt genegeerd.
- `--headroom +5` en `--headroom 05` worden als 5 geaccepteerd (`headroom_accepts_a_plus_sign_and_leading_zero`).
- Een gebruiksfout voor een argument dat geen UTF-8 is, is de eigen tekst van clap, niet de geëscapete weergave.
- "Niet gepubliceerd en geen git-tag" is een repositoryfeit. Geen test controleert het.
- De systeemvereisten in `README.md` (glibc 2.34 voor een releasebuild op Linux x86_64) zijn gemeten op deze machine; de CI-job die het op Ubuntu 22.04 controleert staat onder Niet uitgevoerd.
- De README-claims zonder test, of met maar een deel van een test, zijn in de tabel hieronder gemarkeerd.

## Claim → test

Elke claim van `README.md`, met de test die hem dekt. De namen komen uit de run van 2026-11-19, aangevuld met de nieuwe tests van 2026-11-20. De claimtekst en de testnamen staan zoals in [QUALITY.md](QUALITY.md) (Engels en code). "Gap" betekent dat geen test het bewijst, of maar een deel.

| # | Claim (English, as in README.md) | Test | Status |
| --- | --- | --- | --- |
| C1 | One `REPORT` line is one request | `claim_c1_one_report_line_is_one_request` | tested |
| C2 | GB-seconds are memory × billed ms / 1024 / 1000 | `claim_c2_gb_seconds_are_memory_times_billed_over_1024_over_1000` | tested |
| C3 | The Duration field is not used for the cost | `claim_c3_duration_field_is_not_used_for_the_cost` | tested |
| C4 | Init Duration is not added; it marks a cold start | `claim_c4_init_duration_is_not_added_to_billed_duration` | tested |
| C5 | Requests are 0.20 USD per million on both architectures | `claim_c5_requests_are_twenty_cents_per_million_on_both_architectures` | tested |
| C6 | x86_64 rate and the pricing-page example | `claim_c6_x86_duration_rate_is_the_pricing_page_example` | tested |
| C7 | arm64 rate is not exactly 80 percent | `claim_c7_arm_rate_is_not_an_exact_twenty_percent_discount` | tested |
| C8 | Every report names currency, source, date, region, tier | `claim_c8_every_report_names_source_date_region_and_tier` | tested |
| C9 | The free tier is not subtracted | `claim_c9_free_tier_is_not_subtracted` | tested |
| C10 | Provisioned Concurrency is not priced | `claim_c10_provisioned_concurrency_is_not_priced` | tested |
| C11 | Downsize target and its limits | `claim_c11_downsize_uses_ceil_of_max_used_and_floors_at_128` | tested |
| C12 | Raise target at 90 percent, and raise wins over downsize | `claim_c12_raise_uses_configured_memory_at_ninety_percent` | tested |
| C13 | Advice dollars assume the same billed duration; not a forecast | `claim_c13_advice_is_not_a_forecast` | tested |
| C14 | 1 GB is 1024 MB | `claim_c14_one_gb_is_1024_mb` | tested |
| C15 | One row per function, architecture and memory, in order | `claim_c15_a_row_is_function_architecture_and_memory` | tested |
| C16 | A request id counts once per function, first copy wins | `claim_c16_request_id_is_counted_once_and_the_first_copy_wins` | tested |
| C17 | No AWS SDK, no network connection, rates compiled in | `claim_c17_no_aws_sdk_and_no_network` | gap: manifest and sources only, no network test |
| C18 | Default architecture is x86_64; `--functions` wins | `claim_c18_default_architecture_is_x86_64` | tested |
| C19 | Function name precedence and the name rules | `claim_c19_function_name_precedence` | tested |
| C20 | Memory ranges; the number fields are limited by their text length; `billed duration overflow` | `claim_c20_memory_range_is_128_through_10240`, `billed_duration_and_memory_fields_are_limited_by_their_text_length`, `billed_duration_overflow_is_an_error` | tested |
| C21 | Gzip by magic bytes, every member read, trailing bytes are an error | `claim_c21_gzip_is_recognized_by_magic_bytes_not_the_name` (`src/parse.rs`), `gzip_magic_not_the_file_name`, `gzip_members_are_all_read_and_trailing_bytes_fail`, `corrupt_gzip_is_an_error`, `plain_text_named_gz_still_counts` | tested |
| C22 | Request ids are not printed | `claim_c22_request_ids_are_not_printed` | tested |
| C23 | A bad REPORT line, a non-JSON `{` line, an uncounted marker (also in a field of a document that is not read, and also written with JSON escapes), and message plus events (also nested) are errors without the text | `claim_c23_a_bad_report_line_is_not_printed`, `invalid_json_line_is_an_error_with_its_line_number`, `uncounted_report_marker_in_json_or_bracket_line_exits_2`, `root_object_with_message_and_events_exits_2`, `message_and_events_in_a_nested_event_is_an_error`, `a_second_report_in_another_field_of_a_document_is_an_error`, `an_escaped_marker_in_a_field_that_is_not_read_is_an_error`, `note_line_with_report_text_exits_2_with_empty_stdout` | tested |
| C24 | The total is the exact sum of the row amounts; a sum that is not an exact decimal is an error; not scaled | `claim_c24_the_total_is_the_sum_of_the_rows`, `a_sum_of_exact_rows_that_does_not_fit_exits_2`, `open_finding_half_away_from_zero` | tested |
| C25 | JSON field order, strings, trailing newline | `claim_c25_json_includes_the_source_url_and_the_date`, `json_arm_has_source_and_date` | tested |
| C26 | Marker and field order; `XREPORT` does not match; a marker glued to an invisible character is an error | `claim_c26_a_report_needs_the_marker_and_the_fields_in_order`, `invisible_character_before_report_is_an_error_not_a_silent_drop`, `a_plain_line_with_a_glued_marker_is_ignored_and_a_json_line_with_one_is_an_error`, `a_repeated_field_after_the_fixed_fields_is_trailing_text_and_the_first_counts` | tested |
| C27 | Units `ms` and `MB`; Billed Duration is an integer | `claim_c27_units_are_ms_and_mb_and_billed_duration_is_an_integer` | tested |
| C28 | `--prices` file rules, including padding, 28 digits and unsafe characters in the source | `claim_c28_a_price_file_replaces_the_builtin_rates`, `price_string_trims_only_ascii_whitespace`, `bidi_and_separator_characters_in_source_are_rejected`, `invisible_filler_in_price_source_is_rejected_without_echo` | tested |
| C29 | Timestamp, bracket, BOM, CRLF ignored; JSON `message`, `@message`, `Message` read, also as a pretty-printed document, with its `logGroup` and `architecture` | `claim_c29_a_timestamp_prefix_is_ignored`, `at_message_line_counts_as_a_request`, `pretty_printed_at_message_document_is_read`, `at_message_document_reads_its_architecture_and_log_group` | tested |
| C30 | Errors do not include contents; unsafe characters in paths and arguments; clap keeps its line breaks | `claim_c30_errors_do_not_include_the_file_contents`, `invisible_filler_in_a_file_name_is_replaced`, `bad_arch_with_control_characters_is_escaped`, `unexpected_argument_with_bidi_override_is_escaped`, `headroom_with_cr_and_bidi_override_is_escaped`, `clap_errors_keep_their_line_breaks`, `clap_error_with_escape_and_newline_in_an_argument_has_no_fake_line`, `clap_error_escapes_cr_bidi_and_invisible_characters_on_one_line`, `non_utf8_path_is_shown_as_byte_escape`, `clap_error_for_a_non_utf8_argument_keeps_its_lines_and_has_no_fake_line` | tested |
| L1 | Install from a local path | `install_uses_a_local_path` (`tests/readme.rs`) | tested |
| L2 | The quick start file is `examples/sample-report.log` | `quickstart_heredoc_matches_the_sample_file` (`tests/readme.rs`) | tested |
| L3 | The console examples are what the binary prints | `console_examples_match_the_binary` (`tests/readme.rs`) | tested |
| L4 | The suite has 273 tests | `the_documents_state_the_real_number_of_tests`, `the_counter_counts_attributes_and_nothing_else` (`tests/quality.rs`) | tested: counts test attributes, not the output of `cargo test` |
| L5 | Not published, no git tag, no release archive | none | gap: repository fact, not tested |
| L6 | File, line and directory limits; symlink rules | `file_larger_than_268435456_bytes`, `line_longer_than_1048576_bytes_is_rejected`, `directory_at_64_levels_is_accepted`, `directory_deeper_than_64_levels_is_an_error`, `directory_symlink_is_skipped`, `explicit_symlink_is_followed` | tested |
| L7 | `--prices` and `--functions` files have the size limit, checked before the file is read, must be UTF-8, and must be regular files | `oversized_prices_and_functions_exit_2`, `oversized_prices_and_functions_are_refused_without_reading_them` (Linux only), `prices_and_functions_files_must_be_utf8`, `prices_file_at_the_limit_is_not_refused_for_size`, `a_fifo_as_prices_or_functions_file_is_not_a_file_and_does_not_hang` | tested |
| L8 | A sum that does not fit is `price overflow`, not a panic | `huge_price_is_an_input_error`, `gb_seconds_at_u64_max_does_not_panic`, `a_sum_of_exact_rows_that_does_not_fit_exits_2` | tested |
| L9 | Closed, read-only or full standard output exits 2, also for `--help` and `--version`; `/dev/null` and `1<>/dev/null` exit 0 | `closed_stdout_exits_2_without_panic`, `dev_full_stdout_exits_2`, `read_only_stdout_exits_2`, `help_and_version_to_unwritable_stdout_exit_2`, `dev_null_stdout_exits_0`, `read_write_dev_null_is_not_a_closed_stdout` | tested, Linux only |
| L10 | Other input or usage errors exit 2 | `unknown_option_exits_2`, `missing_file_exits_2`, `no_args_exits_2`, `bad_arch_exits_2`, `headroom_out_of_range_exits_2` (`tests/cli.rs`) | tested |
| L11 | Built-in rates are the first-tier US East figures read on 2026-11-09 | `claim_c8_every_report_names_source_date_region_and_tier` checks the labels, not the page | gap: the page itself is not checked |
| L12 | Limitations list (what is not priced; no extrapolation) | `limitations_name_behaviors_the_tests_cover` (`tests/readme.rs`) checks the text and test names only | gap: partly |
| L13 | System requirements: Rust 1.85, glibc 2.34 for a Linux release build, tests run on Linux only; compiled for Windows, macOS and musl | `licenses_changelog_and_toolchain_match_the_package` checks the Rust version only | gap: glibc and platform statements are not tested |
| L14 | A JSON document, a `--prices` file or a `--functions` file over 4000000 values is refused before it is built, also inside gzip | `a_json_document_with_too_many_values_is_refused_before_it_is_built`, `the_value_budget_keeps_a_document_under_two_gigabytes`, `a_small_gzip_file_cannot_expand_into_too_many_values`, `a_prices_or_functions_file_with_too_many_values_is_refused`, `a_big_text_log_that_starts_with_a_bracket_is_not_counted_as_json`, and the unit tests in `src/budget.rs` | tested: the refusal. The memory of a document at the budget (up to about 3 GB) is measured by hand, see Known limits |
| L17 | `--headroom` takes 0 to 100 and reads `+5` and `05` as 5 | `headroom_accepts_a_plus_sign_and_leading_zeros`, `headroom_out_of_range_exits_2` (`tests/cli.rs`) | tested |
| L18 | A GB-second, a product, an advice amount or a sum that is not an exact 96-bit decimal is `price overflow`, exit 2 | `gb_seconds_quotient_beyond_28_digits_is_an_error`, `a_product_beyond_28_digits_is_an_error`, `a_duration_that_needs_more_than_28_digits_exits_2`, `a_sum_of_exact_rows_that_does_not_fit_exits_2`, `an_advice_amount_that_is_not_exact_exits_2`, `open_finding_half_away_from_zero`, `one_gb_second_at_scale_28_is_exact` | tested |
| L19 | Text in front of a JSON line makes it text; a `# function:` header applies to the whole file; a duplicate RequestId with other values counts once | `text_before_a_json_line_makes_it_text_and_its_marker_does_not_count`, `a_function_header_after_the_first_report_still_names_the_whole_file`, `a_duplicate_request_id_with_other_values_is_only_counted_as_a_duplicate` | tested (pins the documented behavior) |
| L20 | A `logGroup` and a `logGroupName` that differ are named in the error | `two_different_log_group_names_in_one_object_are_named_in_the_error` | tested |
| L21 | An empty-line file of 268435456 bytes takes about 3 s and 526208 KB | none | gap: measured by hand, see Known limits |
| L23 | A gzip file of many empty members takes 39 s; many functions cost memory | none | gap: measured by hand, see Known limits |
| L22 | The workflow checks glibc 2.34 on Ubuntu 22.04 | none | gap: the job is listed under Not run; only the script ran on this machine |
| L15 | `platform.report` records are not read; a pretty-printed object without envelope is rejected; the widest request rate stays exact | `platform_report_records_are_not_read`, `pretty_printed_object_without_a_log_envelope_is_rejected_at_line_1`, `widest_request_rate_is_exact_for_one_and_two_requests` | tested |
| L16 | The documents, the manifest, the sources, the tests, the examples, the workflow and the scripts hold no local path, wrapper option, internal file name, internal tool name or AI system name | `project_files_hold_no_local_paths_or_internal_names`, `the_check_finds_a_planted_reference`, `every_scanned_place_is_read_and_a_word_planted_there_is_found` (`tests/docs_clean.rs`) | tested |
| L24 | The text report says it is an estimate (one `Estimate from list prices` line, an `estimated_usd=` line with the total) and the JSON has `estimate`: true, `estimate_note` and `totals.estimated_usd` | `text_output_labels_the_total_as_an_estimate_once`, `json_output_says_it_is_an_estimate` (`tests/cli.rs`) | tested |
| L25 | A price file dated more than 180 days before `--as-of` (or today) gives a `warning:` line with its date on standard error and exit 0; exactly 180 days gives none; a bad `--as-of` exits 2 | `prices_older_than_180_days_warn_on_stderr_and_exit_0`, `the_price_age_limit_is_exactly_180_days`, `the_built_in_prices_are_checked_against_as_of`, `a_bad_as_of_date_exits_2_and_is_escaped` (`tests/cli.rs`), `days_from_date_counts_calendar_days` (`src/money.rs`) | tested: the tests pass `--as-of`, none reads the clock for its result |
| L26 | The shapes the AWS documentation shows (the `filter-log-events` document with tabs, `Init Duration` and `XRAY`, plain `aws logs tail` lines, CRLF, gzip, log groups with `-`, `_`, digits and capitals, pages that overlap, empty pages) are priced to the exact amount | the nine tests of `tests/documented_shapes.rs` | tested: shapes written from the documentation as remembered, no network, no real export. Sources named in the test: https://docs.aws.amazon.com/lambda/latest/dg/monitoring-cloudwatchlogs.html, https://docs.aws.amazon.com/cli/latest/reference/logs/filter-log-events.html and https://docs.aws.amazon.com/AmazonCloudWatch/latest/logs/S3ExportTasks.html. Not compared with the live pages in this record (no network), so no check date is claimed |
| L27 | `--quiet` prints the result and errors only (no warning on standard error, stdout unchanged); `--verbose` adds a `reading` line per argument and a `done:` line on standard error; both together exit 3 | `help_lists_quiet_and_verbose`, `quiet_with_verbose_is_a_usage_error_with_exit_3`, `quiet_keeps_the_result_and_drops_the_warnings_on_stderr`, `quiet_still_prints_errors`, `verbose_adds_progress_on_stderr_and_keeps_stdout`, `verbose_escapes_the_path_it_reports` (`tests/cli.rs`) | tested |
| L28 | `--md` prints the same amounts as the text and the JSON report as a Markdown table, with the estimate line; a `|`, a backslash and a line break in a cell are escaped; `--md` with `--json` is a usage error | `markdown_amounts_equal_the_text_and_the_json_amounts`, `markdown_and_json_together_are_a_usage_error`, `markdown_escapes_a_pipe_and_a_line_break_in_a_cell` (`tests/cli.rs`, the last on Unix only) | tested |
| L29 | SIGINT exits 130 with the line `lambda-bill: interrupted by SIGINT` and an empty standard output; the handler is Unix only | `sigint_exits_130_with_a_message_and_no_output` (`tests/cli.rs`, Linux only, a real SIGINT sent with `kill`) | tested on Linux; a SIGINT during the write of the report is not tested |
| L30 | `--output FILE` writes the report atomically (temporary file in the same directory, then rename), replaces an existing regular file, refuses a symlink, a directory and a device, creates no directory, leaves no temporary file, and exits 2 with one `cannot write` line on failure; a SIGINT or a failed run keeps the old file | `output_writes_the_report_to_a_file_and_nothing_to_stdout`, `output_replaces_an_existing_file_and_says_so_in_verbose`, `output_in_a_missing_directory_exits_2_and_creates_nothing`, `output_to_a_directory_exits_2_and_leaves_no_temporary_file`, `output_does_not_follow_a_symlink`, `output_to_a_device_is_refused`, `a_failed_run_keeps_the_existing_output_file`, `sigint_keeps_the_existing_output_file` (`tests/cli.rs`; the symlink test on Unix, the device and SIGINT tests on Linux) | tested; a full disk is not tested |
| L31 | Every JSON document has `schema_version` (1) as its first key, as ci-bill does; no error path prints JSON, so none can lack it | `every_json_output_has_schema_version_as_its_first_key`, `a_json_run_that_fails_prints_no_json_at_all` (`tests/cli.rs`) | tested: the second test also passes without the field, it pins that errors print no JSON |

De handleiding wordt gecontroleerd door `help_long_options_match_the_man_page_and_unknown_option_is_not_a_panic` (`tests/handbook_help.rs`): ze bevat de `--help`-tekst letterlijk, en zowel de hele pagina als de sectie OPTIONS hebben dezelfde lange opties als `--help`.
| L32 | No test depends on the system clock: every test that runs the binary passes a fixed `--as-of` | the helper `fixed_clock` in `tests/cli.rs`, `console_examples_match_the_binary` and `every_example_command_matches_its_expected_output` (`tests/readme.rs`); `price_age_*` tests keep their own dates | tested: proof by a copy with the clock 400 days ahead, see the header |
| L33 | `--version` prints the crate version, and a debug build in a git checkout adds the commit | `version_is_the_crate_version_and_a_debug_build_may_add_the_commit` (`tests/cli.rs`) | tested |
| L34 | The README has the sections of the documentation standard, in order | `readme_sections_follow_the_documentation_standard` (`tests/readme.rs`) | tested |
| L35 | The quick start writes only into a new temporary directory | `quick_start_writes_nothing_into_the_repository` (`tests/readme.rs`) | tested |
| L36 | The prices section names the source and the date of the price file; the maintenance section names Issues and security | `prices_section_names_the_source_and_date_of_the_price_file` (`tests/readme.rs`) | tested |
| L37 | The examples README has four commands whose output is the expected file | `every_example_command_matches_its_expected_output` (`tests/readme.rs`) | tested |
| L38 | The Dutch README has the same sections, console blocks, claims and figures as the English one | `the_dutch_readme_says_the_same_as_the_english_one` (`tests/readme.rs`) | tested: the prose is not compared word by word |
| L39 | The release workflow builds five archives with the license listing | none | gap: the workflow is listed under Not run; `cargo about generate` was run once by hand |
