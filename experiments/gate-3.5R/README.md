# Gate 3.5R: replication only

Data: 2026-09-15

## Pergunta

A replicação verificou três observações do Gate 3.5:

1. AYA mais rápido, com menos calls e menos tokens.
2. Qualidade visual potencialmente inferior.
3. Nenhuma vantagem observada em supervisão humana.

Nenhuma feature do AYA MCP foi desenvolvida. O trabalho ficou restrito ao apparatus do experimento, seis execuções, consolidação e publicação.

## Desenho congelado

Foram executados três runs A e três runs B na ordem randomizada:

```text
A, B, A, B, B, A
```

A ordem foi gerada com `shuf` alimentado por 32 bytes de `/dev/urandom` antes do início. O hash da seed foi preservado, mas a seed bruta não; a ordem observada é auditável, não reproduzível a partir do hash.

- A: agente -> wrapper neutro -> Blender Agent Bridge -> Blender.
- B: agente -> o mesmo wrapper neutro -> thin AYA layer -> o mesmo Blender Agent Bridge -> Blender.

Todos os runs válidos usaram:

- `openai-codex/gpt-5.6-terra`;
- thinking `xhigh`;
- Blender 5.1.2;
- CallMeJones Blender Agent Bridge 0.5.6 sem modificação;
- a mesma máquina 4090;
- o mesmo `.blend` inicial;
- o mesmo prompt byte a byte;
- o mesmo wrapper `call` byte a byte;
- budget de 20 minutos e 25 Blender calls;
- Eevee, 1000 x 650, PNG RGBA, AgX Medium High Contrast.

Hashes comuns:

```text
source.blend  06f3473dfd4ef5379eb5606426a1a85f001471ecf0ee1b9388038cc47a5a2de5
prompt.txt    0cd687fd9f82f928f9ff33cfd2201869634eed4b342ac3e595f447eb3ae3b26c
call          7c2fce41ac02fb2d7f1b3bcb23e7afd8bb415c21f0b7fa59d729417ee537165a
```

O prompt nunca identificou A ou B. A rota existia apenas no manifest privado remoto abaixo do wrapper neutro. `results/equality-manifest.json` registra os hashes de prompt, wrapper, sync helper e source em cada run.

## Incidente anterior à sequência válida

Uma primeira tentativa foi invalidada e preservada fora do conjunto analisado. Três defeitos do apparatus foram encontrados:

- um loop alimentado por stdin terminou depois do primeiro run;
- Blender 5.1 rejeitou o enum antigo `BLENDER_EEVEE_NEXT`;
- o validador reconhecia somente convenções de nomes dos candidatos anteriores.

O run afetado foi marcado como inválido. Toda a sequência de seis runs foi recriada com novos identificadores, mesma ordem A/B já randomizada e configuração corrigida antes do novo início. Nenhum ajuste ocorreu entre os seis runs válidos.

Artefatos da tentativa inválida foram preservados em:

```text
/home/aya/tmp/aya-mcp-gate35r-20260914-invalid-v1
```

## Resultados por run

A identidade visual foi revelada somente depois das duas avaliações cegas.

| Ordem | Rota | ID cego | Wall s | Tokens totais | Bridge calls | Tool calls | Tool failures | Visual | Utilizável | Validação semântica |
|---:|:---:|---|---:|---:|---:|---:|---:|---:|:---:|:---:|
| 1 | A | iris | 316,676 | 358.108 | 8 | 21 | 6 | 74 | sim | passou |
| 2 | B | quartz | 291,358 | 352.267 | 9 | 21 | 3 | 94 | sim | passou |
| 3 | A | willow | 305,876 | 300.683 | 8 | 19 | 3 | 92 | sim | passou |
| 4 | B | cobalt | 299,972 | 413.223 | 10 | 25 | 6 | 78 | sim | passou |
| 5 | B | amber | 301,428 | 280.419 | 8 | 20 | 4 | 93 | sim | passou |
| 6 | A | cedar | 307,478 | 303.941 | 8 | 19 | 4 | 96 | sim | passou |

Todos os runs:

- encerraram o agente com código zero;
- preservaram a fonte;
- produziram candidato derivado;
- produziram exatamente três renders iniciais e três finais;
- realizaram um ciclo inspect -> correct;
- mantiveram a configuração de render;
- tiveram zero intervenção humana e zero segundos humanos durante a execução;
- foram considerados utilizáveis para continuidade AYA pelos dois avaliadores cegos.

## Comparação por mediana

| Métrica | A mediana | B mediana | Delta B menos A |
|---|---:|---:|---:|
| Qualidade visual | 92 | 93 | +1 |
| Wall time | 307,478 s | 299,972 s | -7,506 s |
| Input tokens | 55.265 | 71.393 | +16.128 |
| Output tokens | 14.315 | 13.800 | -515 |
| Cache read tokens | 236.032 | 267.136 | +31.104 |
| Total incluindo cache | 303.941 | 352.267 | +48.326 |
| Bridge calls | 8 | 9 | +1 |
| Tool calls totais | 19 | 21 | +2 |
| Tool failures | 4 | 4 | 0 |
| Bridge failures | 2 | 2 | 0 |
| Execuções bpy | 3 | 3 | 0 |
| Intervenção humana | 0 | 0 | 0 |
| Tempo humano efetivo | 0 s | 0 s | 0 s |

O B foi 2,4% mais rápido pela mediana, mas o ganho foi pequeno e os ranges se sobrepõem. Em sentido contrário, B usou 15,9% mais tokens totais, uma bridge call a mais e duas tool calls a mais pela mediana.

## Dispersão e ranges

| Métrica | A range | B range |
|---|---:|---:|
| Wall time | 305,876 a 316,676 s, amplitude 10,800 | 291,358 a 301,428 s, amplitude 10,070 |
| Visual | 74 a 96, amplitude 22 | 78 a 94, amplitude 16 |
| Tokens totais | 300.683 a 358.108, amplitude 57.425 | 280.419 a 413.223, amplitude 132.804 |
| Bridge calls | 8 a 8 | 8 a 10 |
| Tool calls | 19 a 21 | 20 a 25 |
| Tool failures | 3 a 6 | 3 a 6 |

Médias observadas:

- wall A 310,010 s, B 297,586 s;
- visual A 87,333, B 88,333;
- tokens A 320.911, B 348.636;
- bridge calls A 8, B 9.

Com somente três observações por grupo, estes números descrevem o conjunto e não estabelecem significância estatística.

## Qualidade visual e continuidade AYA

Os identificadores e a ordem dos seis candidatos foram randomizados novamente antes da avaliação. Os outputs dos dois avaliadores isolados foram salvos às 00:54:56Z e 00:55:06Z; o primeiro agrupamento A/B foi gerado depois, às 00:58:35Z. O arquivo de protocolo foi registrado pós-execução como transcrição dos prompts já enviados, não como pré-registro. Casey Reas recebeu somente os contact sheets finais e a rubrica congelada:

- conformidade 0 a 40;
- legibilidade 0 a 20;
- clareza espacial 0 a 20;
- completude 0 a 10;
- baixo trabalho residual 0 a 10.

`evaluation/final-contact-evaluator-manifest.json` fixa ordem, identificadores e SHA-256 dos seis arquivos efetivamente apresentados.

Ranking cego:

```text
cedar 96
quartz 94
amber 93
willow 92
cobalt 78
iris 74
```

Um segundo avaliador cego, orientado à continuidade operacional AYA, classificou os seis candidatos como utilizáveis sem reconstrução. Os dois candidatos de menor score exigiriam entre 15 e 60 minutos de refinamento visual; os demais, nenhum ou menos de 15 minutos.

A qualidade visual inferior de B observada no Gate 3.5 não se repetiu. B teve mediana um ponto maior, com resultados intercalados no ranking. A principal variação visual ocorreu entre runs, não entre rotas.

## Validação técnica

O validador congelado passou diretamente em 1 de 6 candidatos porque ainda dependia de famílias específicas de nomes. Os cinco falsos negativos foram preservados como evidência de fricção do apparatus.

Uma adjudicação pós-run, somente leitura e sem alterar candidatos, normalizou as famílias equivalentes de nomes e verificou:

- IDs exatos P1, P2 e P3;
- IDs exatos F1, F2 e F3;
- IDs exatos S1 a S6;
- superfície única ou conjunto setorial com bounding box de aproximadamente 6 x 3 m;
- configuração de render congelada;
- PNGs 1000 x 650;
- hash do candidato reaberto.

Todos os seis passaram nessa adjudicação semântica. A regra executável está em `results/adjudicate_semantics.mjs`, e os inventários publicados registram tipo, coleção, parent, dimensões e posição de cada objeto. Isso sustenta que os cinco resultados rígidos eram falsos negativos de nomenclatura, sem apagar o defeito do validator congelado.

## Custódia e overhead

A fonte foi preservada nos seis runs. B também verificou o hash antes e depois de cada chamada da thin layer. Os logs brutos de hash por call estão publicados em `results/worker-custody/`.

Tempos internos da thin layer B:

| Run | Bridge s | Worker s | Diferença parcial |
|---|---:|---:|---:|
| quartz | 3,452413 | 3,709336 | 0,256923 s |
| cobalt | 3,616576 | 3,920312 | 0,303736 s |
| amber | 3,309671 | 3,521866 | 0,212195 s |

A diferença parcial ficou perto de 29 ms por chamada. Ela é um limite inferior e não inclui toda a manutenção operacional.

Fricções relevantes:

- duas falhas previsíveis do bridge em cada run durante descoberta de schema e aprovação de script;
- mediana de quatro tool failures em A e B;
- um run B precisou de cinco invocações bpy, contra três nos demais;
- validator naming brittle;
- um ciclo completo inválido do apparatus antes dos seis runs válidos;
- B lançou um processo Python adicional por call.

## O que se reproduziu

| Observação do Gate 3.5 | Replicação |
|---|---|
| B mais rápido | apenas fracamente: 2,4% na mediana |
| B com menos calls | não se reproduziu |
| B com menos tokens | não se reproduziu |
| B visualmente inferior | não se reproduziu |
| B reduz supervisão humana | não se reproduziu; empate em zero |
| B preserva custódia por call | reproduzido |

O efeito original de grande vantagem de eficiência e perda visual não se reproduziu e é compatível com variação entre runs. A replicação sustenta somente uma vantagem estrutural pequena e verificável: custódia, budget e restrição de ferramentas abaixo de uma interface neutra.

## Veredito

**SIMPLIFY**

Não houve inferioridade visual observada em B, e a custódia por call funcionou, mas B não demonstrou vantagem proporcional em autonomia, confiabilidade, tokens ou calls. O caminho direto continuou igualmente autônomo e operacionalmente mais simples. O resultado sustenta uma thin layer mínima, não a expansão da infraestrutura maior.

## Hard stop

Relatório e evidências foram publicados. Nenhuma arquitetura simplificada, novo gate, schema, hardening, sandbox, adapter ou DCC adicional foi iniciado.
