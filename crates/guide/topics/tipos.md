# Pendrive, cartão, HD externo, SSD e NVMe

Como cada tipo de dispositivo guarda dados, quanto dura e o que dá para saber da saúde de cada um.

## Memória flash × prato magnético

Existem duas grandes famílias:

- **Memória flash** (pendrive, cartão SD/microSD, SSD, NVMe): guarda dados em chips, sem partes móveis. Aguenta quedas, mas cada célula suporta um número limitado de gravações.
- **Prato magnético** (HD tradicional): um disco gira e uma cabeça lê e grava. Aguenta muitas gravações, mas é sensível a impacto, principalmente enquanto está ligado.

Todo dispositivo tem um **controlador**: um pequeno processador com um programa interno (o *firmware*) que decide onde cada dado fica de verdade. É esse firmware que é adulterado nos pendrives falsificados.

## Cada tipo

| Tipo | Como é por dentro | Pontos fortes | Pontos fracos | SMART |
|---|---|---|---|---|
| Pendrive | flash barata + controlador simples | pequeno, barato | lento para muitos arquivos pequenos; gasta com o uso | quase nunca |
| Cartão SD/microSD | parecido com o pendrive | ocupa pouco espaço | mesmo do pendrive; muitos falsificados | não |
| HD externo | prato magnético + ponte USB-SATA | barato por GB, bom para backup | frágil a quedas; mais lento | sim, se a ponte repassar |
| SSD externo (SATA) | flash de qualidade + ponte USB-SATA | rápido, resistente a impacto | mais caro | geralmente sim |
| NVMe em gaveta | SSD NVMe + ponte USB-NVMe | o mais rápido | esquenta; depende da ponte | geralmente sim |

## Durabilidade

- **Flash** se desgasta com escritas: um pendrive barato pode durar poucos milhares de ciclos por célula; um SSD bom distribui o desgaste e informa quanto da vida útil já foi usada.
- **HD** se desgasta mecanicamente: horas ligado, partidas do motor e, principalmente, impacto. Um HD que cai ligado pode morrer na hora.
- Nenhum dos dois é eterno. Para dados importantes, mantenha **duas cópias em lugares diferentes**.

## Por que SMART nem sempre aparece

SMART é o “painel de saúde” que o próprio dispositivo mantém (veja o tópico *Saúde do disco e SMART*). HDs e SSDs têm; pendrives e cartões quase nunca. Num disco externo, o SMART passa por uma **ponte USB** (o chip da gaveta ou do cabo adaptador). Se a ponte não repassar os comandos, o SMART fica invisível mesmo existindo. Trocar a gaveta por uma com ponte compatível costuma resolver.

## Gavetas e adaptadores

Uma **gaveta** (case) transforma um HD/SSD interno em externo. A qualidade dela importa:

- define a velocidade máxima (USB 2.0 limita qualquer SSD);
- define se o SMART aparece;
- define se o “desligar com segurança” funciona;
- gavetas para HD de 3,5” precisam de fonte própria.

**Na prática:** para backup, um HD externo serve bem; para levar arquivos no bolso, um pendrive de marca conhecida; para velocidade, um SSD externo ou NVMe numa boa gaveta.

**Cuidado:** desconfie de pendrive e cartão com capacidade enorme e preço baixo. É o lugar onde a fraude de capacidade mais acontece.
