# Anchor Escrow - Bootcamp Hackathon Global 2026

Escrow program (Opcao A) em Anchor para dois participantes depositarem tokens SPL e liberar os fundos somente depois que os dois confirmarem.

## Repositorio publico

- https://github.com/gusjjpv/desafio-solana-escrow-anchor-2026

## Program ID (Devnet)

- Program ID: `4PVBfd185KQ7UV3y1h2jR5ydAP9LeB317PNn8aS6gccT`

Status atual: chave sincronizada e pronta para deploy, mas o deploy em devnet depende de saldo na wallet deployer.

Wallet deployer atual:

- DGfKSzB3uuqpBqkuAVcogH5ZH587QxZ1jf9fawyJYBdC

## O que o programa faz

Fluxo principal:

1. `initialize_escrow`: cria a conta PDA do escrow e a vault token account PDA.
2. `deposit`: cada parte deposita sua quantidade obrigatoria na vault.
3. `confirm_deposit`: cada parte confirma seu proprio deposito (somente apos completar o valor exigido).
4. `release_funds`: libera o total depositado para o beneficiario somente quando as duas confirmacoes existem.

## Instrucoes on-chain

- `initialize_escrow(seed, amount_party_one, amount_party_two, beneficiary)`
- `deposit(amount)`
- `confirm_deposit()`
- `release_funds()`

## Estrutura

- `programs/escrow/src/lib.rs`: programa Anchor com validacoes, PDAs e CPI do SPL Token.
- `tests/escrow.ts`: suite basica cobrindo deposito + confirmacao dupla + liberacao.
- `Anchor.toml`: configuracao do workspace Anchor.

## Como rodar localmente

### 1) Requisitos

- Rust + Cargo
- Solana CLI
- Anchor CLI
- Node.js + npm

### 2) Instalar dependencias JS

```bash
npm install
```

### 3) Gerar/sincronizar Program ID

```bash
anchor keys sync
```

### 4) Build

```bash
anchor build
```

### 5) Testes

```bash
npm run anchor:test
```

## Deploy em devnet

Configure sua wallet e endpoint devnet, depois:

```bash
solana config set --url https://api.devnet.solana.com
anchor build
anchor deploy --provider.cluster devnet
```

Em seguida:

1. Copie o Program ID retornado no deploy.
2. Atualize o README na secao Program ID.
3. Rode `anchor keys sync` e confirme se `declare_id!` e `Anchor.toml` estao alinhados.

## Notas de seguranca

- Uso de `checked_add` para evitar overflow.
- Validacao de partes autorizadas para deposito e confirmacao.
- Validacao de mint/beneficiario na liberacao.
- Bumps de PDA armazenados em conta para reuso.
