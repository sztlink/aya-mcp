# Gate 3.5: Real Blender Value Proof

Data: 2026-09-14

Pergunta única:

> AYA MCP permite delegar trabalho real no Blender com menos supervisão humana e valor suficiente para justificar seu overhead?

## Correção factual de 2026-09-15

A avaliação visual foi produzida por um subagente LLM isolado com rubrica congelada, não por Casey Reas ou outro avaliador humano. O registro anterior nomeava uma persona e não publicava os dois contact sheets exatos. Ele permanece visível no histórico Git; o `main` atual registra o método real e publica os bytes avaliados.

## Escopo congelado

Comparação executada na mesma máquina, com o mesmo agente, modelo, thinking, briefing, bridge, fonte sintética e budget:

- Arm A: agente -> Blender Agent Bridge -> Blender.
- Arm B: agente -> wrapper experimental AYA Worker -> o mesmo Blender Agent Bridge -> Blender.

Não houve intervenção humana durante nenhum braço. A fonte foi preservada e cada braço operou em uma cópia própria.

O braço B testou a fronteira Worker mínima e não integrou o Public MCP de produção. O Receipt contextual foi selado após a execução a partir dos logs e artefatos preservados. Portanto, este gate não mede o overhead completo de uma workcell pública distribuída.

## Bridge escolhido

Foi usado CallMeJones Blender Agent Bridge 0.5.6 sem modificação de fonte.

Motivos:

- funciona com Blender 4.2 ou superior;
- expõe inspeção, captura e Python/bpy amplo;
- usa MCP stdio para um bridge HTTP localhost dentro do Blender;
- não exige vendoring nem gateway DCC próprio;
- possui menos superfície operacional que um stack DCC completo.

Snapshots:

- bridge release: `v0.5.6`;
- snapshot de fonte consultado: `140f831689adc4571178fcf80a60d82e23c0c514`;
- ZIP do bridge: `b16756274c26f64c34c4e78f4d68e09753068445817566d10414411d7b48efb7`;
- Blender 5.1.2 Linux x64: `aaccb355f50183979b698bcce7467103a76261b5fa59f4972295842662a285fb`.

Tudo foi instalado de modo portátil na 4090 Render Server sob:

```text
/home/aya/tmp/aya-mcp-gate35-20260914
```

Nenhum perfil pessoal do Felipe foi usado.

## Briefing

A cena deveria conter:

- uma superfície principal de aproximadamente 6 x 3 m;
- seis setores técnicos;
- exatamente três projetores físicos rotulados;
- três frustums visíveis e coloridos;
- collections nomeadas, unidades métricas, materiais e iluminação;
- três renders iniciais;
- inspeção visual autônoma;
- pelo menos uma correção concreta;
- três renders finais;
- candidato salvo sem sobrescrever a fonte.

Budget por braço:

- até 25 chamadas Blender MCP;
- até 20 minutos;
- zero intervenção humana;
- bpy/Python amplo autorizado.

O briefing integral está em `fixtures/briefing.md`.

## Resultado bruto

| Métrica | Arm A | Arm B |
|---|---:|---:|
| Wall time | 358,149 s | 241,011 s |
| Intervenções humanas | 0 | 0 |
| Model calls | 17 | 15 |
| Tool calls do agente | 20 | 19 |
| Falhas de tool do agente | 0 | 2 |
| Chamadas ao bridge | 12 | 8 |
| Falhas do bridge | 0 | 0 |
| Execuções Python/bpy | 3 | 3 |
| Ciclos inspect/correct | 1 | 1 |
| Crashes | 0 | 0 |
| Renders iniciais/finais | 3 / 3 | 3 / 3 |
| Input tokens | 67.452 | 78.377 |
| Output tokens | 16.697 | 11.243 |
| Cache read tokens | 429.824 | 200.192 |
| Total incluindo cache | 513.973 | 289.812 |
| Custo reportado pelo harness | US$ 0,4212 | US$ 0,3317 |
| Validação técnica | passou | passou |
| Avaliação visual cega por LLM, não humana | 93/100 | 86/100 |

Deltas do Arm B:

- 117,138 s mais rápido, redução de 32,7%;
- quatro chamadas a menos no bridge, redução de 33,3%;
- 224.161 tokens a menos incluindo cache, redução de 43,6%;
- 10.925 input tokens a mais;
- duas falhas autocorrigidas no uso do wrapper;
- sete pontos a menos na avaliação visual cega feita por LLM, não por avaliador humano.

As duas falhas do Arm B foram de integração no shell, não do Blender: uma tentativa inicial de usar um placeholder não permitido e uma checagem local de um caminho que existia somente na 4090. O agente se recuperou sem ajuda.

O wrapper do Arm B lançou oito processos curtos. Seus timers internos somaram 3,633004 s contra 3,396480 s do bridge interno, diferença parcial de 0,236524 s, aproximadamente 29,6 ms por chamada. Esse é somente um limite inferior: o timer original excluiu o startup do Python e o primeiro hash da fonte. Também não inclui o custo de desenvolver e manter a superfície adicional.

O setup compartilhado observado, da criação da raiz temporária ao início do Arm A, foi 521,586 s. A atenção humana dentro desse intervalo não foi instrumentada separadamente. Essa é uma limitação do protocolo. O setup humano e as intervenções de Felipe durante os braços foram zero.

## Custódia e validação

Fonte comum:

```text
391b334775e6259f4f46d1413216a1cce12a6b1fb9101bb82ca54ede2fa9c410
```

Candidatos:

```text
Arm A  ec52f20b3e4d9d9d11705b944152235d4a30bc3f7d1773afd7a7fbc348f475b5
Arm B  7a76a847cc4feab14b6caf21d432ac4228897ffeb9f1565a9f5ce031f4c2dc8a
```

Ambos preservaram a fonte. O Arm B verificou o hash antes e depois de cada chamada do Worker. A validação final reabriu cada candidato diretamente do `.blend` salvo, confirmou o hash do arquivo carregado e contou exatamente três projetores, três frustums e seis setores por marcadores de nome congelados para este briefing. O Receipt contextual do Arm B passou:

- schema validation do Score, Lease e Receipt;
- binding contextual Score/Lease/Receipt;
- Receipt event chain;
- digest do Receipt;
- rehash de todos os artefatos e evidências.

A garantia continua sendo `contract_only`. Não há claim de sandbox, `process_isolated` ou `os_enforced`.

## Avaliação de utilidade

Ambos os candidatos são tecnicamente completos e utilizáveis como estudo sintético.

Um `isolated model evaluator with frozen rubric`, operado como subagente LLM e não como avaliador humano, recebeu Candidate X e Candidate Y sem saber qual fluxo operacional cada um representava. Após revelar o mapeamento, X era o Arm A e Y era o Arm B.

| ID cego | Contact sheet exato | Score LLM | Rota revelada depois |
|---|---|---:|:---:|
| X | [`blind/X/final-contact.png`](evaluation/blind/X/final-contact.png) | 93 | A |
| Y | [`blind/Y/final-contact.png`](evaluation/blind/Y/final-contact.png) | 86 | B |

Os bytes são os inputs exatos da avaliação, sem rerender, recompressão ou regeneração:

```bash
cd experiments/gate-3.5/evaluation
sha256sum -c SHA256SUMS
```

A publicação transforma os dois inputs em um corpus reavaliável por modelos futuros ou avaliadores humanos, sem depender do pacote SMB.

O Arm A foi considerado mais útil porque a relação projetor, frustum e setor é mais direta, os rótulos são mais consistentes e as perspectivas preservam melhor a geometria espacial. O Arm B tem boa legenda e divisão dimensional, mas tipografia e alguns vínculos de feixe são menos imediatos.

Assim, Receipt e custódia não transformam o Arm B em vencedor. A melhora de velocidade e economia coexistiu com perda visual e duas fricções de integração. Como a supervisão humana empatou em zero, o benefício principal proposto pelo AYA não foi demonstrado.

## Limitações

- Ordem fixa A depois B, com possível efeito de cache aquecido no Arm B.
- Uma execução por braço, sem poder estatístico.
- Geração do modelo é estocástica mesmo com configuração equivalente.
- O agente sabia qual comando operacional usar em cada braço.
- O Arm B mede um Worker mínimo, não o fluxo Public MCP completo.
- Score, Lease e Receipt foram reconstruídos e selados pós-execução, não admitidos e emitidos online pelo Worker. Os hashes iniciais, finais e logs Worker preservados foram vinculados ao Receipt, mas isso continua sendo evidência contextual, não prova de admissão prévia.
- Setup humano compartilhado não foi instrumentado em segundos.
- A avaliação visual foi feita por LLM sobre contact sheets, não por avaliador humano nem por navegação interativa nos `.blend`.
- Não houve calibração óptica, fotometria ou validação em instalação real.
- O bridge aceitava raw bpy no localhost sem token durante a janela do experimento. Qualquer processo local poderia tentar invocá-lo; isso é compatível apenas com a declaração `contract_only`.

## Reprodução

Na 4090, copie os scripts deste diretório para `$GATE35_BASE/scripts` e execute:

```bash
export GATE35_BASE="$HOME/tmp/aya-mcp-gate35-20260914"
./scripts/install_gate35.sh
export BLENDER_USER_CONFIG="$GATE35_BASE/profile/config"
export BLENDER_USER_SCRIPTS="$GATE35_BASE/profile/scripts"
export BLENDER_USER_CACHE="$GATE35_BASE/profile/cache"
export BLENDER_USER_EXTENSIONS="$GATE35_BASE/profile/extensions"
export GATE35_BASE_BLEND="$GATE35_BASE/base/source.blend"
"$GATE35_BASE/install/blender-current/blender" --background --factory-startup \
  --python "$GATE35_BASE/scripts/create_base.py"
./scripts/prepare_arms.sh
./scripts/start_arm.sh A
```

No AYA2, execute `scripts/run_agent_arm.sh A`, pare o Arm A, valide o candidato salvo e repita para B. Os scripts de chamada agora aplicam os limites de 20 minutos e 25 chamadas. A execução histórica foi encerrada abaixo dos dois limites, mas a primeira versão do apparatus confiava no prompt e no timeout do processo do agente para aplicar o budget.

A validação abre novamente o `.blend` salvo antes de contar e registrar seu hash. Na 4090, para cada braço encerrado:

```bash
export GATE35_ARM_ROOT="$GATE35_BASE/runs/A"
export GATE35_CANDIDATE_PATH="$GATE35_ARM_ROOT/work/candidate.blend"
"$GATE35_BASE/install/blender-current/blender" --background --factory-startup \
  --python "$GATE35_BASE/scripts/validate_scene.py"
```

No AYA2, `collect_gate35_evidence.sh /caminho/para/evidence-root` monta a estrutura local a partir da 4090 e dos logs do agente. Depois execute:

```bash
node experiments/gate-3.5/scripts/finalize-gate35.mjs /caminho/para/evidence-root
```

O finalizador deriva eventos dos tipos de chamada observados, relê hashes iniciais e finais, vincula a validação ao hash do candidato reaberto e usa o horário real da selagem pós-run.

## Veredito

**SIMPLIFY**

O experimento não justifica continuar expandindo a arquitetura atual. Ele justifica preservar somente uma camada fina de custódia, budget e superfície de ferramentas restrita sobre bridges existentes. Public MCP distribuído, supervisão mais ampla e novos contratos devem permanecer congelados até que um novo teste mostre ganho de supervisão ou qualidade, sem depender de Receipt para compensar resultado visual inferior.

## Próximo gesto, após decisão humana

Reduzir a hipótese a um Worker fino e repetir apenas um teste pareado randomizado com duas execuções por braço e Receipt online. O critério de avanço deve exigir simultaneamente:

1. zero intervenção humana;
2. qualidade visual não inferior;
3. economia consistente de tempo ou chamadas;
4. custódia automática sem selagem pós-execução.

HARD STOP: não iniciar esse gesto, Gate 4, hardening, sandbox, TouchDesigner ou After Effects nesta rodada.

## Evidências

O pacote completo inclui:

- fonte original;
- candidatos A e B;
- seis renders por braço;
- contact sheets iniciais e finais;
- logs JSONL completos do agente;
- logs MCP e Worker;
- métricas brutas derivadas;
- validações técnicas;
- Score, Lease e Receipt do Arm B;
- hashes e inventário de arquivos;
- protocolo, ordem e hashes da avaliação visual.

Binários e logs grandes ficam fora do Git. O pacote durável fica no SMB indicado abaixo. Os scripts `run_agent_arm.sh` e `finalize-gate35.mjs` registram a execução e a consolidação, mas a réplica também requer Blender, o bridge externo e o perfil descartável descritos neste documento.

```text
\\192.168.15.169\AYA WORKS\_SISTEMA\AYA-MCP-GATE-3.5-20260914
```
