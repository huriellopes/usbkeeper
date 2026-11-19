# Tabela de partições: MBR × GPT

Antes do sistema de arquivos vem a tabela de partições: o mapa que diz onde cada parte do disco começa e termina.

## O que é uma tabela de partições

Pense no disco como um livro. A **tabela de partições** é o sumário: diz que o capítulo 1 vai da página 1 à 200 e o capítulo 2 da 201 à 500. Cada capítulo é uma **partição**, e cada partição tem o seu próprio **sistema de arquivos** (FAT32, exFAT, NTFS, ext4…).

Existem dois formatos de sumário: o antigo **MBR** e o atual **GPT**. Quem lê o disco (computador, TV, som do carro) precisa entender o formato usado.

## Comparação

| | MBR | GPT |
|---|---|---|
| Idade | de 1983 | dos anos 2000 |
| Tamanho máximo do disco | 2 TiB (com setores de 512 bytes) | praticamente ilimitado |
| Quantidade de partições | 4 partições primárias | 128 partições |
| Cópia de segurança do mapa | não | sim, no fim do disco |
| Inicialização do computador | BIOS antiga | UEFI (computadores atuais) |
| Compatibilidade com aparelhos | ótima (TV, som, carro, impressoras) | boa em computadores; ruim em aparelhos antigos |

**2 TiB** é cerca de 2,2 TB. Num HD de 4 TB com MBR, só os primeiros ~2,2 TB podem ser usados.

## Quando usar cada um

**Use MBR quando:**

- o pendrive ou HD vai ser usado numa **TV**, som de carro, videogame antigo ou outro aparelho;
- o disco tem até 2 TB;
- você vai usar `--max-size` para aproveitar só a parte real de um pendrive falsificado (veja abaixo).

**Use GPT quando:**

- o disco tem **mais de 2 TB**;
- o disco vai dar boot em um computador com **UEFI**;
- você precisa de mais de 4 partições.

O usbkeeper escolhe **MBR até 2 TiB e GPT acima disso**, que é o mais compatível para pendrives e HDs externos. Você pode trocar manualmente na hora de formatar.

## Por que pendrive falsificado exige MBR

O GPT grava uma **cópia de segurança do mapa no último setor** do disco. Num pendrive falsificado, esse “último setor” anunciado não existe de verdade: a gravação some ou, pior, cai por cima do começo do disco e estraga o mapa principal. Por isso, quando você limita a formatação à parte real com `--max-size`, o usbkeeper usa sempre MBR e recusa GPT.

**Na prática:** na dúvida, deixe o padrão do usbkeeper. Só mude para GPT se o disco for maior que 2 TB ou se for para dar boot num computador moderno.

**Cuidado:** trocar a tabela de partições apaga o acesso a tudo o que havia no disco. É uma formatação completa do mapa.
