# Projektspezifikation: Feather — Desktop-Client für Pterodactyl

> Stand: 19. Juli 2026 · Version 0.2 (ergänzt um Deploy-Flow v2, Datenmodell, Architektur und die finalen Grundsatzentscheidungen)

---

## 1. Vision & Motivation

Eine Desktop-App für **Linux und Windows**, die für Pterodactyl das ist, was GitHub Desktop für Git/GitHub ist: Man wählt lokal einen Projektordner, hält Versionsstände als Commits fest und deployed sie per Klick auf den eigenen Pterodactyl-Server — und verwaltet die Server nebenbei gleich mit (Start/Stop, Status, Konsole).

**Marktumfeld:** Für Pterodactyl existieren Mobile-Apps, Web-Erweiterungen und mit `MythicalLTD/Pterodactyl-Desktop` auch eine Desktop-App auf Client-API-Basis — aber **kein Client mit Deploy-/Versionierungs-Workflow**. Das ist das Alleinstellungsmerkmal von Feather.

**Persönliches Ziel des Projekts:** Ein eigenes Produkt betreiben, das andere Leute aktiv nutzen, auf das sie sich verlassen, zu dem Feedback kommt (GitHub Issues) und auf dessen Updates sich Nutzer freuen (Releases + Auto-Updater).

**Zielgruppe:** Pterodactyl-Nutzer — Betreiber von Game-Servern, Discord-Bots und kleinen Diensten, die regelmäßig Dateien/Code auf ihre Server bringen.

---

## 2. Grundsatzentscheidungen (final)

| Entscheidung | Wahl | Begründung |
|---|---|---|
| Name | **Feather** | Anspielung auf den Pterodactyl-Daemon „Wings"; „verlässlicher Helfer". GitHub-Check 07/2026: im Pterodactyl-Umfeld unbelegt |
| Framework | **Tauri 2** (Rust-Kern, Webview-Frontend) | Kleine Binaries, sauber auf Windows + Linux, Rust ideal für Git/Zip/Upload |
| Frontend | **Svelte 5** + TypeScript + Vite | Kleine Bundles, wenig Boilerplate, eingebaute Reaktivität für Live-Daten |
| Lizenz | **MIT** | Maximal einfache Adoption, üblich im Ökosystem |
| Git-Anbindung | **git2 (gebündeltes libgit2)** | Keine Abhängigkeit vom System-Git des Nutzers |
| Design | **Dark Dev-Look** (VS-Code-artig), Akzent Violett | Zielgruppe sind Entwickler/Selfhoster |
| Panels | **Erstmal ein Panel**, intern als Liste | Multi-Panel später nur UI-Update, keine Migration |
| Versionierung | **Echtes Git unter der Haube** | Rollback trivial, Diffs gratis, Power-User können parallel mit Git arbeiten |
| Upload-Weg | **Pterodactyl File-API** | Signierte Upload-URL + Entpacken über die Client-API |
| Upload-Umfang | **Immer alles** + `.deployignore` + **Manifest-Löschung** | Sicher & simpel; gelöschte Dateien werden gezielt nachgezogen (s. 6.3) |
| Deploy-Ziel | **Pro Projekt wählbar** | Standard Server-Root, optional Unterordner |
| Build-Schritt | **Optional pro Projekt** | Textfeld „Befehl vor Deploy", standardmäßig aus |
| Nach dem Deploy | **Pro Projekt einstellbar** | Neustart oder nur Benachrichtigung |
| Credentials | **System-Schlüsselbund**, Datei-Fallback | Windows Credential Manager / Secret Service; ohne Schlüsselbund verschleierte Fallback-Datei (s. README) |
| Teststrategie | **Mock-Panel im Repo** | Client-API-Subset als eigenes Crate; Kern + CI voll automatisch testbar |
| Lizenz/Modell | **Komplett Open Source** (GitHub) | Issues, Community, Stars als Motivation |
| Updates | **Eingebauter Auto-Updater** | Tauri-Updater + GitHub Releases (ab M5, braucht Signatur-Schlüsselpaar) |

---

## 3. Kern-Features (nach Priorität)

1. **Deploy-System** — Projektordner per Klick auf den Server bringen (Herzstück)
2. **Commit-Historie & Rollback** — Versionsstände festhalten, alte Version per Klick wieder deployen
3. **Server-Verwaltung** — Power-Aktionen (Start/Stop/Restart/Kill), Status auf einen Blick
4. **Live-Konsole** — Konsolen-Output streamen, Befehle senden

## 4. Zusatzfeatures für Version 1 (Bau-Reihenfolge nach Aufwand)

1. **CPU/RAM live auf den Kacheln** — über denselben Websocket wie die Konsole
2. **Desktop-Benachrichtigungen** — Tauri bringt das mit
3. **Auto-Backup vor jedem Deploy** — Backup-API, mit Rotation (s. 6.3)
4. **Datei-Browser für den Server** — teuerstes Feature, als letztes

Dazu: **`.deployignore`** (gitignore-Syntax) — bei „immer alles hochladen" praktisch Pflicht.

---

## 5. UI-Konzept

- **Hauptlayout:** Dashboard mit **Server-Kacheln** (Muster aus dem Panel bekannt)
- **Pro Kachel:** Servername, Status-Punkt (online/startet/offline), CPU- und RAM-Balken live, Deploy-Button als zentrales Element, Power-Button, Icons für Konsole/Historie/Dateien
- **Kopfleiste:** App-Name, Verbindungsstatus zum Panel, Einstellungen
- **Fußleiste:** Git-Status des aktiven Projekts (z. B. „3 Commits seit letztem Deploy"), Benachrichtigungs-Status
- **Deploy-Zustand:** Fortschritt direkt auf der Kachel („Backup erstellt · Upload 68 %")
- **Farbwelt:** Dunkles Theme, Akzent Violett (#8b5cf6), Statusfarben Grün/Orange/Rot

---

## 6. Technische Architektur

### 6.1 Repository-Struktur

```
crates/wingman-core/   Panel-API-Client (reqwest), Deploy-Engine, git2, Zip,
                       .deployignore, Datenmodell — KEINE Tauri-Abhängigkeit
crates/mock-panel/     Mock der Pterodactyl Client-API (axum) für Tests + Dev
src-tauri/             Tauri-2-Shell: Fenster, IPC-Commands, Schlüsselbund
src/                   Svelte-5-Frontend
```

Der strikte Kern/Shell-Split hält die gesamte Logik ohne GUI testbar
(`cargo test` gegen das Mock-Panel, auch in CI und Cloud-Umgebungen).

### 6.2 Pterodactyl Client-API (`/api/client`)

Alles Nötige ist mit einem Client-API-Key verfügbar: Serverliste, Power-Aktionen,
Websocket (Konsole + Live-Ressourcen + Backup-Events), Datei-Verwaltung
(auflisten, signierte Upload-URL, entpacken, löschen), Backups. Reserve-Option
für sehr große Projekte: die API liefert SFTP-Zugangsdaten pro Server (nicht v1).

**Websocket-Details (M2):** Das Konsolen-Token läuft nach ~10–15 Minuten ab und
muss per API erneuert werden; Reconnect mit Backoff; ein Socket pro Server.

### 6.3 Deploy-Flow (v2)

```
1. Git-Commit des Projektordners (App legt beim Einrichten ein Repo an, falls keins existiert)
2. Optional: Build-Befehl ausführen (Shell im Projektordner, Output live in der UI,
   Abbruch bei Exit-Code ≠ 0)
3. Auto-Backup anstoßen UND auf Abschluss warten (Backup-API ist asynchron;
   die App pollt den Backup-Status, bis `completed_at` gesetzt ist — robuster
   als das Websocket-Event, weil der Deploy keinen offenen Socket braucht).
   Eigene Backups heißen "feather-pre-deploy-<zeitstempel>" und werden rotiert:
   ist das Backup-Limit des Servers erreicht, wird das älteste eigene gelöscht;
   bei Limit 0 wird der Schritt mit Hinweis übersprungen. Pro Projekt abschaltbar.
4. Zip packen — Ausschlüsse laut .deployignore (gitignore-Syntax, ignore-Crate);
   .git/ und .deployignore selbst sind immer ausgeschlossen
5. Upload über signierte URL; Größenlimits des Panels/Proxys (oft 100 MB) werden
   als klare Fehlermeldung gemeldet
6. Entpacken in Server-Root oder konfigurierten Unterordner;
   danach das hochgeladene Zip per Delete-API vom Server entfernen
7. Manifest-Löschung: Dateien, die im Manifest des letzten Deploys standen und im
   aktuellen fehlen, werden per Delete-API entfernt (Serverdaten außerhalb des
   Projekts — Welten, Datenbanken — bleiben unangetastet)
8. Je nach Projekt-Einstellung: Server neustarten oder Desktop-Benachrichtigung
```

Jeder Schritt meldet Fortschritt als Event an die UI (Kachel-Visualisierung).
Schlägt ein Schritt fehl, wird klar benannt, welcher — das Backup aus Schritt 3
ist der Rettungsanker.

### 6.4 Rollback-Flow

```
1. Nutzer wählt alten Commit aus der Historie
2. `git archive` des Commits in einen Temp-Ordner
   (KEIN Checkout im Projektordner — uncommittete Änderungen bleiben unberührt)
3. Normaler Deploy-Flow ab Schritt 2, Quelle ist der Temp-Ordner
```

### 6.5 Datenmodell (App-Config-Verzeichnis, JSON)

- `panels.json`: Liste (v1: ein Eintrag) `{id, name, base_url}` —
  der API-Key liegt **nur** im System-Schlüsselbund (Service `feather`, Key = Panel-id)
- `projects.json`: `{id, name, local_path, panel_id, server_uuid, target_dir,
  build_command?, post_deploy: "restart"|"notify", auto_backup}`
- Pro Projekt: Deploy-Historie mit `{commit_hash, timestamp}` des letzten Deploys
  und dem Datei-**Manifest** — speist Manifest-Löschung und die Fußleiste
  („N Commits seit letztem Deploy" = `git rev-list <letzter-deploy>..HEAD`)

---

## 7. Meilenstein-Plan

**Prinzip:** Früh etwas Lauffähiges haben.

- **M1 — Verbindung & Dashboard** ✅: Panel-URL + API-Key (Schlüsselbund), Serverliste, Kacheln mit Status, CPU/RAM
- **M2 — Server fühlt sich echt an** ✅: Power-Buttons (Kill zweistufig), Websocket → Live-Konsole mit Befehlseingabe + CPU/RAM live, Token-Refresh/Reconnect mit Backoff
- **M3 — Deploy-Kern** ✅: Projektordner verknüpfen (Ordner-Picker), Zip → Upload → Entpacken → Zip-Cleanup, `.deployignore`, Manifest-Löschung, Zielordner, Verhalten nach Deploy, Desktop-Benachrichtigungen, Fortschritt auf der Kachel
- **M4 — Versionierung** ✅: git2-Integration (Repo-Init beim Verknüpfen, Auto-Commit vor Deploy), Commit-UI + Historie mit „deployed"-Marker, Rollback (Tree-Archive in Tempdir, Working Tree unberührt), Auto-Backup mit Rotation (nur eigene `feather-pre-deploy-*`), optionaler Build-Befehl mit Live-Output, Fußleiste „N Commits seit letztem Deploy"
- **M5 — Komfort & Release** ✅: Datei-Browser (navigieren, Ordner anlegen, löschen), Auto-Updater (GitHub Releases + latest.json; Signatur-Schlüsselpaar wird vom Betreiber erzeugt, siehe docs/RELEASING.md), Release-Workflow für Windows (NSIS) + Linux (AppImage, .deb), Ein-Zeilen-Installer für Linux (install.sh)

---

## 8. Open Source & Community

- GitHub-Repo öffentlich, Feedback über Issues + Discussions
- Releases über GitHub Releases, ausgeliefert per Auto-Updater
- Changelog pro Release — „Vorfreude auf Updates" ist erklärtes Projektziel
- Launch-Kanäle: Pterodactyl-Discord, r/selfhosted, r/admincraft
- Sprache von UI, Code und README: **Englisch** (internationale Zielgruppe); diese Spezifikation bleibt Deutsch

## 9. Offene Punkte

- [ ] Logo/Icon entwerfen (aktuell Platzhalter: violettes „W")
- [ ] Erstes Release veröffentlichen: Updater-Schlüsselpaar erzeugen, Secrets setzen, Tag pushen (docs/RELEASING.md), dann Launch in den Community-Kanälen (Abschnitt 8)
- [x] M5 (Datei-Browser, Auto-Updater, Release-Pipeline, Installer) — 19.07.2026
- [x] M4 (Versionierung) — 19.07.2026
- [x] M3 (Deploy-Kern) — 19.07.2026
- [x] M2 (Websocket, Power-Aktionen, Konsole) — 19.07.2026
- [x] Name final festlegen (Feather, Verfügbarkeit geprüft 07/2026)
- [x] Frontend-Framework wählen (Svelte 5)
- [x] Open-Source-Lizenz wählen (MIT)
- [x] Repo aufsetzen + M1 (19.07.2026)
