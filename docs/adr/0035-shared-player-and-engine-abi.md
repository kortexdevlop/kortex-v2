# 0035 — Lecteurs partagés et moteurs wasm à ABI neutre
**Statut** : accepté (2026-10-21) par l'utilisateur (« fait tout ça d'un coup sans t'arrêter », après le plan en huit étapes 20.1–20.8) ; moteur d'exécution `wasmtime` choisi par l'utilisateur le 2026-10-22 (« privilégie wasmtime, c'est mieux même si c'est plus lourd ») après mesure. Écart explicite à MASTER_PROMPT §7.3 (« build par outil : engine.wasm + UI web + binaire desktop ») : l'interface web et le binaire desktop ne sont plus **compilés** par outil, ils sont **assemblés**. RULES §1.4 : cette instruction de l'utilisateur est la dérogation ; RULES §3.1/§4.4 et CLAUDE.md sont mis à jour en conséquence.

## Contexte (constaté dans le code le 2026-10-21)
- `xtask build` génère une crate Dioxus par outil et la compile avec `dx` (≈ 1,3 Mo de wasm par outil, interface et moteur ensemble) ; `xtask build-desktop` compile un binaire natif Dioxus par outil et par cible ; `xtask/src/worker.rs` compile un moteur `wasm-bindgen` par outil. 42 outils = 42 compilations web + 42 par cible + 42 moteurs ; la release complète prend ≈ 21 minutes sur le runner Mac.
- L'interface d'un outil est déjà pilotée par le contrat : les 42 outils sont `ui = "auto"` (`AutoTool`), aucun n'a d'interface personnalisée.
- Le point de jonction existe : `kortex_ui::Runner(fn(ValueMap, ValueMap) -> RunFuture)`. Les hôtes web et desktop ne diffèrent que par ce `Runner`, le contrat (`tool::CONTRACT`), les locales et un titre de fenêtre.
- Le protocole headless desktop (`--engine`, JSON-RPC sur stdio : `run`, `contract`, `shutdown`) est consommé par Flow, Mind et la CLI (`ProcessRunner`) ; l'hôte de moteurs web (`web/run-host/host.js`) lance `/<id>/engine/worker.js`.
- Le moteur wasm actuel n'est pas neutre : il dépend de `wasm-bindgen` (glue JavaScript, valeurs JS) et, via `kortex-runtime → kortex-platform`, de code navigateur (`js-sys`, `getrandom` `wasm_js`) qu'il n'appelle pas mais dont il importe les symboles.

## Décision

### 1. Un moteur = un `engine.wasm` à ABI neutre v1
Fichier produit par `xtask build-engine`, identique pour le web, le desktop, Flow, Mind et la CLI. Pas de `wasm-bindgen`, pas de glue propre à l'outil.

Exports (tous obligatoires) : `kx_abi() -> u32` (= 1), `kx_alloc(len: u32) -> u32`, `kx_free(ptr: u32, len: u32)`, `kx_run(ptr: u32, len: u32) -> u64` (poids fort = pointeur de la réponse, poids faible = longueur ; la réponse est à libérer par `kx_free`), plus la mémoire `memory`.

Imports fournis par l'hôte, module `kx_host` : `now_ms() -> u64` (horloge UTC), `fill_random(ptr: u32, len: u32) -> u32` (0 = succès ; CSPRNG système), `is_cancelled() -> u32`, `progress(milli: u32)`. **Tout autre import** (par exemple des symboles résiduels de bibliothèques tierces) doit être satisfait par l'hôte avec une fonction qui échoue si elle est appelée : un moteur qui l'atteint répond `worker_failed`, il ne compromet pas l'hôte.

Trame (requête et réponse) : `u32 LE` longueur du JSON, le JSON UTF-8, puis les blobs binaires bout à bout. Le JSON reprend la forme serde actuelle de `Value` (`{"Text":"x"}`, `{"Int":3}`, …) ; un fichier devient `{"File":{"name","mime","blob":i}}` et ses octets sont le blob d'indice `i` (la liste des longueurs est dans `"blobs"`). La requête est `{"inputs":{…},"params":{…},"blobs":[…]}`, la réponse `{"Ok":{…},"blobs":[…]}` ou `{"Err":{"code","detail"}}`. Les octets ne passent donc jamais en JSON : pas de régression pour les gros fichiers par rapport au chemin web actuel.

Évolution : `abi_version` n'augmente que pour une rupture ; un import ou un export facultatif s'ajoute sans changer la version. Un hôte refuse un moteur dont `kx_abi()` est supérieur à ce qu'il sait faire.

Dépendances : `kortex-runtime` gagne la fonctionnalité `engine-abi` (cible wasm32 uniquement) qui remplace horloge, entropie et `yield_now` par les imports `kx_host` : le moteur n'atteint plus aucun code de `kortex-platform`. Les exports sont générés par une macro de `kortex-runtime` (`export_engine!`) ; la logique (`handle`) est du Rust ordinaire testé en natif.

### 2. Paquet d'outil (`.kxp`), indépendant de la plateforme
Octets : `KXPAYLD1`, longueur du corps (u64 LE), SHA-256 du corps (32 octets), corps = nombre de fichiers (u32) puis, pour chacun, longueur du nom (u32), nom, longueur (u64), données. Fichiers : `tool.toml` (contrat tel quel), `locales/fr.toml`, `locales/en.toml`, `glyph.svg`, `engine.wasm`. Le lecteur vérifie le SHA-256 avant d'utiliser quoi que ce soit.

### 3. Lecteur desktop et assemblage
- Crate `kortex-player` (fonctionnalité `desktop`) : fenêtre Dioxus, `AutoTool`, hôte wasm (`wasmtime`, crate `kortex-engine-host`), service `--engine` conservé à l'identique (méthodes `run`, `contract`, `shutdown`, délai et annulation de `kortex-runtime::native`).
- Le lecteur **réserve une zone** dans son image (statique de capacité fixe, en-tête et marque reconnaissables, 4 Mio par défaut) : un binaire d'outil = le lecteur dont cette zone a été remplie par le paquet, sur place, sans changer la taille ni la structure du fichier. L'assembleur la trouve par sa marque (aucune analyse de format), vérifie qu'elle est unique et écrit le paquet. Sur macOS, la CI re-signe ensuite en ad hoc (`codesign --force --sign -`) comme aujourd'hui. Le lecteur lit la même zone en mémoire au démarrage.
- Mesures qui justifient ce choix (2026-10-21, macOS Intel) : coller le paquet après le lecteur fait refuser `codesign` (« main executable failed strict validation ») avant comme après signature ; ajouter une section avec `llvm-objcopy` donne une signature valide mais un binaire tué au lancement (code 137). Ces deux méthodes sont écartées. Sur Apple Silicon la signature est obligatoire et aucune CI ne peut l'y exécuter : seule une méthode conforme par construction est acceptable.
- Titre de fenêtre, langue, modes, thème, enregistrement et annulation : repris du desktop actuel (`build_desktop_one`).

### 4. Lecteur web
- Même crate (`web`), compilée une seule fois par `dx` : publiée sous `https://run.<domaine>/_player/`. Chaque outil garde son adresse `/<id>/` : `index.html` (copie de celui du lecteur, identifiant de l'outil en métadonnée), `tool.toml`, `locales/*.toml`, `engine/engine.wasm`, `sw.js` (cache hors ligne de l'outil et du lecteur).
- Le `Runner` web exécute **toujours** le moteur dans un Worker : `/engines/kx-worker.js?tool=<id>` (un script unique, module, qui charge `/<id>/engine/engine.wasm`, fournit `kx_host`, convertit les `Uint8Array` en blobs et renvoie la même forme `{Ok…}`/`{Err…}` qu'aujourd'hui). L'hôte de moteurs de Flow/Mind (`host.js`) lance le même script : plus de `worker.js` par outil.
- Origine, CSP, Stage, adresses et protocole de messages : inchangés.

### 5. Publication et installation : formes inchangées
Les paquets desktop restent des archives `manifest.json` + binaire par outil et par cible, signées dans l'index (ADR 0007/0034) ; seul leur **mode de fabrication** change (assemblage en CI au lieu d'une compilation). Les clients publiés (CLI 0.2.0, Store) n'ont rien à changer. `kortex run`, Flow et Mind desktop continuent d'appeler `--engine`. Les archives web restent une par outil (fichiers du dossier de l'outil) ; le lecteur partagé est publié comme entrée web `player` du même mécanisme (`package-web`, `restore-web`).

### 6. Règles de dépendance et de reconstruction
- `xtask changed` : une modification d'un outil ne reconstruit que son moteur, son paquet et ses binaires assemblés ; une modification de `kortex-player`, `kortex-ui`, `kortex-platform`, `kortex-runtime`, `kortex-contract`, `kortex-i18n`, `kortex-engine-host`, `kortex-payload` ou des jetons reconstruit le lecteur (donc tous les binaires assemblés) ; une modification de `kortex-runtime` ou du macro reconstruit aussi tous les moteurs.
- Un outil n'ajoute plus aucune crate de compilation d'interface : `tools/<u>/<n>/` garde `tool.toml`, moteur Rust, locales, contenu, glyph, tests.

## Hors périmètre (décisions séparées)
Schéma d'adresses des 10 000 pages, catalogue découpé, outils « en données seulement », hébergement des paquets, levée du gel à 42 outils. Aucun nouvel outil n'est ajouté par ce chantier.

## Alternatives écartées
- **Binaire natif par outil** (état actuel) : une compilation par outil et par cible, inadapté au volume visé.
- **Paquet collé en fin de fichier** : `codesign` refuse le fichier (mesuré).
- **Section ajoutée par `llvm-objcopy`** : binaire tué au lancement (mesuré).
- **wasmi** (interpréteur en Rust pur, lecteur d'environ 1,4 Mo) : mesuré le 2026-10-21 sur `image.compress` (PNG de 5,4 Mo) à 2,1 s contre 0,24 s en natif, soit environ 9 fois plus lent ; écarté au profit de `wasmtime`, qui compile en code machine (0,32 s, environ 1,3 fois le natif). L'ABI ne change pas : le moteur d'exécution est isolé dans `kortex-engine-host`.
- **Assemblage à l'installation par le Store/la CLI** : oblige à embarquer la signature dans les clients et change leur contrat ; l'assemblage en CI garde les paquets signés et les clients inchangés.
- **Champ `engine = "native"`** : non ajouté tant qu'aucune mesure n'en montre le besoin (YAGNI) ; un ADR le créera si un outil l'exige.

## Conséquences et limites
- Un binaire d'outil pèse environ la taille du lecteur plus 4 Mio de zone réservée ; la zone est incompressible à l'installation mais compresse presque entièrement dans l'archive téléchargée.
- Un moteur dont le paquet dépasse la capacité de la zone est refusé à l'assemblage avec un message actionnable ; la capacité se règle à la compilation du lecteur.
- Les moteurs wasm sont compilés en code machine par `wasmtime` (Cranelift) : mesuré à 1,3 fois le natif sur `image.compress`. Contreparties : environ 12 Mo de plus dans chaque binaire (le lecteur) et environ 0,5 s de compilation du module au premier usage ; le lecteur la lance en arrière-plan au démarrage pour que la fenêtre s'ouvre aussitôt, et `--engine` la paie à la première requête. Le lecteur se compile avec le profil `player` (dépendances optimisées en vitesse, LTO fin). Une modification de la version majeure de `wasmtime` est un changement d'hôte, pas d'ABI.
- Windows, Linux et Apple Silicon ne sont vérifiés que par la CI ; la recette locale couvre macOS Intel (RULES §7.1 : preuves distinctes).
