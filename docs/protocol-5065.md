# Protocolo mínimo 5065

Este documento registra a primeira fatia de integração entre o cliente e o servidor. Os IDs abaixo são os IDs usados pela base atual do CoEmu e precisam ser confirmados contra o cliente 5065 real durante a integração.

## Transporte

- TCP.
- Cada pacote possui `u16 length` + `u16 packet_id` + payload.
- Campos numéricos usam little-endian.
- O cliente usa `CQCipher` e o servidor usa `TQCipher`; são as duas metades assimétricas do cipher legado.
- O tamanho inclui os quatro bytes do cabeçalho.

## Fluxo inicial

| Direção | ID | Pacote | Estado |
| --- | ---: | --- | --- |
| Cliente → Auth | 1051 | `MsgAccount` | implementado no `gravak-protocol` |
| Auth → Cliente | 1055 | `MsgConnectEx` | parser implementado |
| Cliente → Game | 1052 | `MsgConnect` | implementado no gravak-protocol |
| Game → Cliente | 1004 | `MsgTalk` | implementado no gravak-protocol |
| Game → Cliente | 1006 | `MsgUserInfo` | implementado no `gravak-protocol` |
| Game → Cliente | 1110 | `MsgMapInfo` | implementado; `flags` ocupa 1 byte neste wire |
| Cliente ↔ Game | 1005 | `MsgWalk` | implementado no probe |
| Cliente → Game | 1001 | `MsgRegister` | implementado para personagem de teste |
| Cliente → Game | 1010 | `MsgAction` | `SendLocation` implementado |
| Game → Cliente | 1033 | `MsgData` | parser implementado |

## `MsgAccount` — 1051

Payload de 48 bytes:

1. `username`: 16 bytes, texto ASCII preenchido com zero;
2. `password`: 16 bytes, senha no formato `TQPassword`/RC5;
3. `realm`: 16 bytes, texto ASCII preenchido com zero.

## `MsgConnectEx` — 1055

Resposta de sucesso, 28 bytes:

1. `token`: `u64`;
2. `game_server_ip`: 16 bytes;
3. `game_server_port`: `u32`.

Resposta de rejeição, 24 bytes:

1. `reserved`: `u32`;
2. `rejection_code`: `u32`;
3. `message`: 16 bytes.

## Teste manual

Com o auth server disponível em `127.0.0.1:9958`:

```powershell
cd R:\gravak-co\client
cargo run --release --locked -p gravak-probe
```

O `gravak-probe` usa o token e o endereço retornados para abrir a conexão do game server, envia `MsgConnect` 1052, registra o personagem quando recebe `NEW_ROLE`, reconecta, lê `MsgUserInfo`/`MsgMapInfo` e envia uma caminhada `MsgWalk`.

## Cliente gráfico

O sandbox permanece offline por padrão. Para ativar o fluxo online, use `--online` ou informe `--auth`:

```powershell
cargo run --release --locked -p conquer-sandbox -- --assets R:\5065 --online --auth 127.0.0.1:9960 --username test1 --password 123456 --realm CoEmu
```

A rede roda em uma thread própria e entrega ao loop da janela os estados de conexão, personagem, mapa e posição inicial. Cada passo concluído pelo `Player` já é convertido para a direção 0–7 e enviado como `MsgWalk`. Ecos válidos confirmam a fila de passos; `Invalid Location` e `MsgAction::Teleport` limpam a fila e reposicionam o jogador na coordenada do servidor.

O `9960` é específico do ambiente local atual, onde o auth do monorepo roda ao lado do auth legado em `9958`.
