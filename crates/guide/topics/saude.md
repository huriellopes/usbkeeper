# Saúde do disco e SMART

O que os números do SMART significam, por que pendrives quase nunca os têm e quando é hora de trocar o disco.

## O que é SMART

SMART é um registro que o próprio HD ou SSD mantém sobre si mesmo: horas ligado, temperatura, setores com defeito, erros de comunicação. O usbkeeper lê esses números e traduz num veredito: **saudável**, **atenção** ou **falhando**, sempre dizendo o motivo.

## Os números que importam

| Atributo | O que significa | Quando preocupar |
|---|---|---|
| 5: setores realocados | trechos defeituosos que o disco já trocou por áreas de reserva | qualquer valor acima de zero merece atenção; se cresce com o tempo, o disco está morrendo |
| 197: setores pendentes | trechos que falharam na leitura e esperam ser trocados | acima de zero: copie seus dados logo |
| 198: incorrigíveis | erros que o disco não conseguiu corrigir | acima de zero: dados já podem ter sido perdidos |
| 199: erros de CRC | erros na comunicação pelo cabo ou pela ponte USB | costuma ser o **cabo ou a gaveta**, não o disco; troque o cabo |
| 9: horas ligado | idade de uso | informativo; HDs costumam passar de 30 mil horas |
| 194: temperatura | temperatura atual | acima de 60 °C é quente demais |
| NVMe: vida útil usada | quanto da resistência de escrita já foi gasta | perto de 90% ou mais, planeje a troca |
| NVMe: erros de mídia | erros de integridade dos dados | acima de zero merece atenção |

Os setores **realocados**, **pendentes** e **incorrigíveis** são os mais importantes: indicam defeito físico.

## Por que o pendrive não tem SMART

Pendrives e cartões usam controladores simples e baratos, que quase nunca implementam SMART. Em HDs e SSDs externos, o SMART existe, mas passa pela **ponte USB** da gaveta. Se a ponte não repassar os comandos, o usbkeeper avisa “o adaptador USB não repassa comandos SMART”.

Para esses casos existe a **varredura de superfície**: o usbkeeper lê o disco inteiro, sem gravar nada, e aponta blocos ilegíveis e trechos lentos demais (sinal de áreas degradadas que o controlador está relendo várias vezes).

## Quando trocar

- **Falhando:** copie seus dados **agora** e substitua o disco. Não use para backup.
- **Atenção com realocados ou pendentes crescendo:** copie seus dados e planeje a troca. Rode a varredura para ver a extensão do dano.
- **Atenção só por CRC:** troque o cabo ou a gaveta e acompanhe; se parar de crescer, o disco está bem.
- **Pendrive com blocos ilegíveis na varredura:** não confie mais nele.

**Na prática:** rode a saúde de vez em quando no HD de backup. O histórico do usbkeeper guarda cada leitura, então você consegue ver se os números estão piorando.

**Cuidado:** “saudável” não é garantia de que o disco não vai falhar amanhã. Backup em dois lugares continua sendo a única proteção real.
