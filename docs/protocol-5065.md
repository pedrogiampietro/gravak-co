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
| Game → Cliente | 1006 | `MsgUserInfo` | próximo passo |
| Game → Cliente | 1110 | `MsgMapInfo` | próximo passo |
| Cliente ↔ Game | 1005 | `MsgWalk` | próximo passo |

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

O gravak-probe já usa o token e o endereço retornados para abrir a conexão do game server, envia MsgConnect 1052 e lê a primeira resposta MsgTalk 1004.
