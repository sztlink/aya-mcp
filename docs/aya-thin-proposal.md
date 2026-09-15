# AYA Thin

## Redesign proposal, sem implementação

Status: proposal para decisão humana.

Base experimental:

- Gate 3.5: uma execução por rota.
- Gate 3.5R: três execuções por rota, prompt neutro e avaliação cega.
- Resultado: acesso direto e thin AYA foram igualmente autônomos e utilizáveis.
- Ganho cognitivo ou de supervisão da rota AYA: não demonstrado.
- Valor observado: custody, budget, tool restriction e evidence.

Este documento não autoriza código, Gate 4, confinement, provenance, contratos ou novos adapters.

## 1. Tese mínima

AYA Thin não é uma plataforma de agentes, supervisor distribuído, gateway DCC ou protocolo de workcells.

AYA Thin é uma capa local e pequena em torno de um bridge existente:

```text
briefing humano
  -> fonte preservada + candidato separado
  -> um comando neutro call
  -> política fina de hash, budget, allowlist e log
  -> bridge existente, sem modificação
  -> DCC
  -> candidato + evidência plana
```

O bridge continua responsável por descoberta, execução DCC, dispatch na main thread, captura e linguagem nativa. AYA Thin não replica nenhuma dessas funções.

## 2. Garantias mínimas

### 2.1 Source custody

Comportamento mínimo:

1. calcular SHA-256 da fonte antes do run;
2. criar uma cópia separada para o candidato;
3. marcar o snapshot de input como read-only quando a plataforma permitir;
4. verificar o hash da fonte antes e depois de cada call;
5. verificar novamente ao finalizar;
6. invalidar o resultado se o hash mudar.

Limite obrigatório de linguagem:

> Em `contract_only`, isso detecta divergência de conteúdo nos pontos de verificação. Não detecta uma mutação transitória que seja restaurada antes do próximo hash e não torna a fonte imutável contra um processo com a mesma autoridade do usuário.

Imutabilidade adversarial exigiria ACL, mount read-only ou confinement. Isso não faz parte de AYA Thin neste proposal. Neste nome, `immutable source` significa uma invariante de custódia verificada em pontos definidos, não enforcement contínuo de sistema operacional.

### 2.2 Derived candidate

Comportamento mínimo:

1. iniciar `work/candidate` como cópia byte a byte da fonte;
2. abrir somente o candidato no DCC;
3. nunca fornecer ao agente um destino de promoção;
4. aceitar como output somente o path de candidato declarado no início;
5. registrar hash inicial e final do candidato;
6. rejeitar o fechamento se o candidato esperado não existir ou se a fonte tiver mudado.

Isso preserva separação de custódia e review. Não tenta provar semanticamente que cada byte do candidato deriva somente da fonte.

### 2.3 Execution budget

Somente dois limites obrigatórios:

- deadline monotônico de wall time;
- contador máximo de calls ao bridge.

Cada call recebe apenas o tempo restante. Ao expirar, o wrapper recusa novas chamadas e encerra somente processos filhos que ele próprio iniciou. O contador deve ser serial ou atômico; o contador shell usado nos experimentos não provou concorrência segura. Não existe lease distribuído, renovação, retomada ou cancelamento remoto.

### 2.4 Restricted tool surface

A allowlist opera em dois níveis quando o bridge oferece um gateway:

- nome da tool externa;
- nome da helper interna presente nos argumentos.

O wrapper não interpreta Python, bpy, ExtendScript ou payload criativo. Se `draft_script` estiver permitido, a autoridade continua ampla e deve ser declarada como tal.

### 2.5 Evidence e log

Evidência mínima:

- `calls.jsonl`: timestamp, tool, helper, duração, resultado, erro e hashes da fonte;
- `run.json`: config efetiva, tempos, contagens, exit code, source hash e candidate hash;
- arquivos de evidência declarados por glob ou path, com SHA-256;
- stdout e stderr do agente e do bridge quando disponíveis.

Não há event chain. Não há assinatura. Não há claim de append-only adversarial. Um `SHA256SUMS` plano é suficiente para consistência interna e transporte.

O apparatus existente já registra metadata, sucesso, erro e hashes parciais, mas não satisfaz toda esta seção: não preserva a resposta completa, não produz um `run.json` genérico e não coleta evidência independente de Blender. Esses itens são requisitos de um Thin eventual, não capacidade pronta.

## 3. Menor interface possível

### Interface humana ou de workflow

Uma única operação conceitual:

```text
run(configuração local, comando do agente)
```

Ela prepara input e candidato, inicia o budget, disponibiliza `call`, executa o agente e fecha o log.

### Interface visível ao agente

Um único comando:

```text
call TOOL JSON_ARGUMENTS
```

O comando:

1. verifica deadline e call count;
2. aplica allowlist nominal;
3. rehash da fonte;
4. encaminha a chamada sem alterar seu conteúdo;
5. registra metadata e resposta;
6. rehash da fonte;
7. devolve a resposta original do bridge.

Não expor ao agente `request`, `status`, `cancel`, `review`, `discover capability`, `execute` ou `capture` como tools AYA. Quando necessárias, discovery, execute e capture continuam sendo tools do próprio bridge, acessadas pelo mesmo `call`.

### Configuração mínima

Um único manifest local, sem schema público, contém apenas:

1. path da fonte;
2. path do candidato;
3. comando ou endpoint do bridge;
4. allowlist de tools e helpers;
5. wall time máximo;
6. calls máximas;
7. paths ou globs de evidência;
8. diretório do run.

O manifest é configuração operacional, não contrato entre autoridades.

## 4. Subconjunto aproveitável do código atual

O recorte abaixo identifica comportamentos parcialmente provados. Não implica copiar arquivos hardcoded sem revisão nem afirma que o Thin já existe.

| Função | Referência atual | LOC relevante aproximada | Estado factual |
|---|---|---:|---|
| Hash antes e depois da call | `experiments/gate-3.5/scripts/aya_worker_call.py` | 35 | provado em pontos de verificação |
| Preparar fonte e candidato | `experiments/gate-3.5/scripts/prepare_arms.sh` | 15 | provado por convenção e cópia inicial |
| Deadline e call count | `aya_call.sh`, `mcp_call.py`, `run_agent_arm.sh` | 30 | provado em execução serial, não concorrente |
| Allowlist de gateway tool | `aya_worker_call.py` | 15 | provada; helper allowlist ainda não existe |
| Log JSONL parcial | `aya_worker_call.py`, `mcp_call.py` | 30 | provado sem resposta completa ou finalização genérica |
| Transporte ao bridge | `mcp_call.py` | 125 | provado como cliente one-shot |
| Validação DCC específica | `validate_scene.py` | 95 | Blender-only e opcional |

Tamanho observado do apparatus atual:

- kernel parcial de política: cerca de 113 LOC;
- caminho callable com cliente MCP: cerca de 238 LOC;
- caminho com preparação e budget externo: cerca de 321 LOC.

Startup, relay e shutdown específicos de Blender não pertencem ao Thin. Devem permanecer no workflow ou no próprio bridge. Incluí-los elevaria o recorte para aproximadamente 440 LOC, mas criaria exatamente o acoplamento DCC que este proposal rejeita.

O recorte de 321 LOC ainda não possui helper allowlist, finalização genérica, `run.json`, coleta genérica de evidência, contenção de paths nem testes completos. Uma implementação Thin exigiria trabalho novo para essas lacunas. Não existe hoje um componente reutilizável AYA Thin.

## 5. O que sai do caminho ativo

| Componente atual | Decisão no proposal | Motivo |
|---|---|---|
| Public/Worker split | congelar com Gate 3 | a replicação não demonstrou valor proporcional para duas autoridades MCP |
| Score schema | congelar | briefing humano continua como arquivo simples, sem admissão protocolar |
| Lease | remover do runtime Thin | deadline e call count são aplicados diretamente no run local |
| Receipt event chain | remover do runtime Thin | consistência interna é atendida por log plano e hashes; não havia âncora externa |
| Synthetic supervisor | congelar integralmente | provou lifecycle sintético, mas não agrega valor ao wrapper real mínimo |
| Cancellation semantics | remover | substituir por timeout local e encerramento best effort dos filhos próprios |
| CapabilityReport | remover | não negociar enforcement; declarar estaticamente `contract_only` |
| Lifecycle states | colapsar | somente exit code e resultado final `completed` ou `failed` |
| Fake DCC | congelar como prova histórica | não pertence ao runtime real |
| Public review tool | remover | review continua fora do wrapper, diretamente sobre candidato e evidência |
| Node schema validator | congelar | AYA Thin não possui contratos públicos para validar |
| Rust workspace ativo | congelar | nenhuma função mínima exige quatro crates ou duas MCPs próprias |
| Gate 3.5 finalizer | congelar | era apparatus e reconstruía contratos pós-run |
| Validator Blender | tornar opcional | específico do briefing e do DCC, não política central |

Congelar significa manter no histórico e nas tags atuais, sem carregar, publicar como runtime recomendado ou continuar expandindo.

## 6. Redução estimada

As categorias atuais se sobrepõem. Os números são ranges de planejamento, não orçamento aprovado.

### Linhas de código

Método do baseline: `wc -l` sobre `.rs`, `.mjs` e `.json` em `crates`, `src`, `bin`, `scripts`, `test` e `spec/v0`, excluindo documentação, experimentos, manifests Cargo, CI e resultados.

- runtime Rust e contratos: 3.497 LOC;
- validator e CLI Node: 313 LOC;
- quatro schemas e dois manifests de tools: 252 LOC;
- testes e harness MCP: 997 LOC;
- total contado: 5.059 LOC.

O apparatus de 321 LOC é piso histórico incompleto, não estimativa de produto. Contabilizando finalização, evidência genérica, helper allowlist, paths e testes, a estimativa prudente para um eventual AYA Thin é:

- runtime: 450 a 800 LOC;
- runtime mais testes focados: 600 a 1.100 LOC;
- redução contra o baseline contado: aproximadamente 78% a 88%.

Meta de contenção caso seja autorizado:

- máximo 800 LOC de runtime;
- máximo 1.100 LOC incluindo testes essenciais;
- nenhum schema público novo.

### Processos

AYA MCP sintético administra Public MCP, Worker MCP, supervisor e fake DCC. O caminho real ainda acrescenta bridge e DCC.

AYA Thin teria:

- um wrapper AYA;
- o bridge existente;
- o DCC.

Redução de papéis de processo controlados pela AYA: de quatro para um, aproximadamente 75%. Se implementado como wrapper one-shot, ainda haverá subprocessos transitórios por call. O proposal não promete reduzir calls ou subprocessos do bridge.

### Configuração

Atual:

- Cargo workspace e quatro crates;
- toolchain Rust;
- dependências Node;
- quatro schemas;
- dois manifests de tools;
- fixtures, Score, Lease, CapabilityReport e Receipt;
- configuração de duas superfícies MCP.

Thin:

- um manifest local;
- uma allowlist dentro do mesmo manifest;
- uma configuração do bridge já existente.

Pela contagem estreita de quatro schemas, dois manifests de tools, cinco manifests Cargo, toolchain, package e workflow CI, o stack atual possui cerca de 14 artefatos ativos de configuração e contrato. Thin precisaria de dois ou três: manifest local, configuração do bridge já existente e um job de teste. Redução estimada nessa contagem: 79% a 86%.

### Tool surface

Atual:

- cinco tools Public;
- três tools Worker;
- bridge separado.

Thin:

- um comando AYA visível ao agente: `call`;
- tools internas continuam pertencendo ao bridge.

Redução da superfície AYA: de oito tools para uma, 87,5%. Gate 3.5R mostrou que isso não implica menos bridge calls: B teve mediana 9, A teve 8.

### Manutenção

Estimativa qualitativa:

- linguagens ativas: Rust + Node + Python/shell para uma linguagem de wrapper, além do bridge externo;
- pipelines de validação: Rust + Node + E2E sintético para uma suíte curta de wrapper;
- componentes com lifecycle próprio: quatro para um;
- contratos versionados ativos: quatro para zero;
- manutenção total: redução indicativa de 60% a 85%, sem denominador operacional observado.

Esse range não deve decidir a alternativa. A manutenção não cai se cada DCC exigir fork próprio. Nesse caso, integrar as quatro funções diretamente aos workflows pode ser menor.

## 7. Testes mínimos, se houver autorização futura

Este proposal não autoriza implementação. Ele apenas define o limite máximo de uma eventual suíte:

1. fonte preservada em sucesso;
2. mutação da fonte invalida o run;
3. candidato ausente invalida o run;
4. deadline recusa nova call;
5. call count recusa excesso;
6. tool ou helper fora da allowlist é recusada;
7. erro do bridge aparece no JSONL e no exit code;
8. evidência declarada é rehashada;
9. candidate path e evidence path não escapam do run;
10. wrapper não altera request ou response do bridge.

Sem fake DCC complexo, lifecycle distribuído, orphan process tests, Receipt chain ou cancelamento remoto.

## 8. Riscos aceitos

AYA Thin permanece `contract_only`:

- fonte read-only pode ser alterada por processo com autoridade suficiente;
- bridge e scripts criativos podem acessar o host;
- allowlist nominal não torna raw Python seguro;
- logs e hashes podem ser reescritos por quem controla o diretório;
- não existe network policy;
- não existe recuperação durável;
- não existe garantia de process-tree cleanup;
- não existe promoção segura embutida;
- não existe ganho cognitivo ou de supervisão demonstrado.

O projeto só é honesto se esses limites permanecerem visíveis.

## 9. Critério para impedir nova expansão

Se AYA Thin for autorizado, qualquer proposta que introduza um dos itens abaixo exige nova decisão humana e deixa de ser Thin:

- segunda MCP própria;
- schema público;
- lifecycle distribuído;
- registry persistente;
- assinatura ou event chain;
- adapter DCC;
- interpretação semântica de código criativo;
- confinement;
- promoção de candidato;
- mais de uma tool AYA visível ao agente;
- mais de 1.100 LOC ativas incluindo testes.

## 10. Alternativa 1: AYA Thin como projeto útil

### Hipótese

Custody, budget, tool restriction e evidence são necessidades repetidas em vários workflows. Uma implementação única reduz duplicação e drift.

### Benefícios

- política uniforme entre bridges;
- formato de log e hashes consistente;
- uma interface neutra para o agente;
- separação clara entre fonte e candidato;
- componente pequeno, testável e publicável;
- bridge permanece externo e substituível.

### Custos e riscos

- mantém um projeto e um nome próprios sem ganho cognitivo demonstrado;
- adiciona processo, configuração e falhas possíveis;
- pode voltar a crescer em direção ao AYA MCP atual;
- continua sem segurança adversarial;
- exige owner e disciplina permanente de escopo.

### Condições que justificam escolher esta alternativa

- vários workflows independentes precisam das mesmas quatro funções;
- o custo esperado de duplicação, drift e correções repetidas supera o custo do wrapper compartilhado;
- evidência uniforme tem valor operacional recorrente;
- existe um owner de manutenção;
- o limite de 800 LOC runtime e uma tool é aceito;
- nenhum workflow precisa de semântica DCC dentro do Thin.

## 11. Alternativa 2: arquivar AYA MCP

### Hipótese

Acesso direto ao bridge já oferece a autonomia necessária. As quatro funções úteis são simples demais para justificar um projeto próprio.

### Forma

- arquivar o repo no estado atual com Gates 3, 3.5 e 3.5R preservados;
- incorporar cópia de fonte, candidato separado, timeout, allowlist e logs diretamente em cada workflow SZTLink;
- manter bridge e DCC como já existem;
- não criar um runtime chamado AYA Thin.

### Benefícios

- nenhuma abstração ou processo adicional;
- menor custo imediato de manutenção;
- cada workflow pode usar convenções nativas do seu DCC;
- elimina risco de nova expansão arquitetural;
- reflete diretamente o resultado experimental de autonomia equivalente.

### Custos e riscos

- lógica de custody e budget pode ser duplicada;
- logs e evidências podem divergir entre workflows;
- correções precisam ser repetidas;
- não existe superfície comum para consumidores externos;
- auditoria transversal fica mais difícil.

### Condições que justificam escolher esta alternativa

- poucos workflows precisam dessas funções;
- não há consumidor externo do wrapper;
- diferenças entre bridges dominam a implementação;
- consistência entre workflows não tem valor suficiente;
- scripts locais pequenos resolvem cada caso sem drift ou correções repetidas relevantes.

## 12. Avaliação neutra

| Critério | AYA Thin | Arquivar e incorporar no SZTLink |
|---|---|---|
| Autonomia demonstrada | igual ao direto | igual ao direto |
| Ganho cognitivo | não demonstrado | não reivindicado |
| Custody uniforme | melhor | depende de cada workflow |
| Budget uniforme | melhor | depende de cada workflow |
| Evidence comparável | melhor | potencialmente divergente |
| Complexidade imediata | maior | menor |
| Risco de expansão | médio | baixo |
| Reuso entre bridges | potencialmente alto | baixo |
| Ajuste específico por DCC | indireto | direto |
| Manutenção com um workflow | pior | melhor |
| Manutenção com muitos workflows | potencialmente melhor | potencialmente pior |
| Reversibilidade | alta | alta |

Nenhuma alternativa possui preferência prévia neste proposal.

A decisão depende de uma comparação operacional, não de um número arbitrário de workflows:

> O valor recorrente de política e evidência uniformes supera o custo estimado de manter um wrapper compartilhado de 600 a 1.100 LOC?

Se a uniformidade evitar drift e correções repetidas reais, AYA Thin pode ser útil sob limites rígidos. Se scripts locais permanecerem menores e estáveis, arquivar AYA MCP e incorporar as quatro funções diretamente tende a ser a forma menor.

## HARD STOP

Proposal concluído. Nenhum código foi escrito, removido ou reorganizado. Nenhuma alternativa foi iniciada. Aguardar decisão de Felipe.
