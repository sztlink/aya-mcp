# Avaliação visual cega por LLM

Correção factual de 2026-09-15: esta avaliação foi produzida por um subagente LLM isolado com rubrica congelada, não por Casey Reas ou outro avaliador humano. O registro anterior nomeava uma persona e permanece no histórico Git.

O avaliador LLM recebeu Candidate X e Candidate Y sem informação sobre qual fluxo operacional cada candidato representava. O mapeamento usado após a avaliação foi X = Arm A e Y = Arm B.

Inputs exatos publicados:

- [Candidate X, score 93](../evaluation/blind/X/final-contact.png)
- [Candidate Y, score 86](../evaluation/blind/Y/final-contact.png)

Os hashes podem ser verificados em [`evaluation/SHA256SUMS`](../evaluation/SHA256SUMS).

| Critério | Arm A | Arm B |
|---|---:|---:|
| Conformidade | 38/40 | 35/40 |
| Legibilidade | 18/20 | 17/20 |
| Clareza espacial | 19/20 | 17/20 |
| Completude | 9/10 | 9/10 |
| Trabalho residual, baixo residual | 9/10 | 8/10 |
| **Total** | **93/100** | **86/100** |

O avaliador LLM considerou o Arm A mais útil. A relação projetor, frustum e setor está mais direta; os rótulos físicos e setoriais são mais consistentes; e as três vistas preservam melhor a geometria espacial. O Arm B tem boa legenda e divisão dimensional, mas tipografia e alguns vínculos de feixe aparecem menores e menos imediatos nas perspectivas.
