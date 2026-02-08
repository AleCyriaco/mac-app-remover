# Mac App Remover

Utilitario para macOS que remove aplicativos e seus arquivos residuais (caches, preferences, logs, containers, etc).

Disponivel em duas versoes: **CLI** para o terminal e **GUI** com interface grafica nativa.

## Funcionalidades

- Lista todos os aplicativos instalados em `/Applications` e `~/Applications`
- Busca aplicativos por nome (case-insensitive)
- Detecta arquivos residuais em 10 diretorios do `~/Library`
- Mostra tamanho do app e total a ser liberado
- Fecha o app automaticamente se estiver em execucao
- Remove o bundle `.app` e todos os arquivos relacionados

## Instalacao

Requer [Rust](https://rustup.rs/) instalado.

```bash
git clone https://github.com/AleCyriaco/mac-app-remover.git
cd mac-app-remover
cargo build --release
```

Os binarios ficam em `target/release/`.

## Uso

### CLI

```bash
# Listar todos os aplicativos
mac-app-remover list

# Buscar por nome
mac-app-remover search chrome

# Remover um aplicativo
mac-app-remover remove "Google Chrome"
```

### GUI

```bash
mac-app-remover-gui
```

A interface possui:
- Barra de busca para filtrar apps
- Lista scrollable com nome e tamanho
- Painel de detalhes com caminho, Bundle ID e arquivos residuais
- Botao de remocao com dialogo de confirmacao
- Log de status em tempo real

## Estrutura do projeto

```
src/
├── lib.rs          # Logica compartilhada (CLI + GUI)
├── main.rs         # Binario CLI
└── bin/
    └── gui.rs      # Binario GUI (egui/eframe)
```

## Dependencias

- [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) - Framework GUI (egui)
- [rfd](https://github.com/PolyMeilex/rfd) - Dialogos nativos

## Licenca

MIT
