# Verificar e reparar

Quando o pendrive ou HD passa a dar erro, pedir “reparo” ou aparecer vazio, a primeira suspeita é o sistema de arquivos; veja como conferir e corrigir sem piorar.

## O que é um sistema de arquivos inconsistente

O sistema de arquivos é o índice que diz onde cada arquivo está (veja *Sistemas de arquivos*). Ele fica **inconsistente** quando uma gravação é interrompida no meio:

- o dispositivo foi arrancado sem **remover com segurança**;
- faltou energia, o cabo mexeu ou o HD externo desligou sozinho;
- o computador travou durante uma cópia.

Os sintomas vão de “o Windows quer verificar a unidade” a pastas com nomes estranhos, arquivos de 0 bytes, ou o dispositivo aparecendo como “não formatado”.

## Verificar × reparar

O usbkeeper separa as duas coisas:

1. **Verificar** apenas lê a estrutura e diz se há problemas. **Não altera nada** e não pede a série. É sempre o primeiro passo.
2. **Reparar** corrige o que encontrou e, logo em seguida, **verifica de novo** para confirmar o resultado. Como altera o disco, pede a confirmação pela série.

Em ambos os casos o dispositivo precisa estar **desmontado**; o usbkeeper desmonta sozinho, ou mostra qual programa está usando o disco.

## O que cada resultado significa

| Resultado | Significado | O que fazer |
|---|---|---|
| sem problemas | a estrutura está íntegra | nada |
| tem problemas | a verificação achou inconsistências | rode o reparo |
| corrigido | o reparo resolveu e a nova verificação saiu limpa | confira seus arquivos |
| ainda tem erros | o reparo não resolveu tudo | veja as dicas abaixo |
| não verificado | o usbkeeper não verifica esse sistema de arquivos | use a ferramenta do sistema de origem |
| a ferramenta falhou | o programa de reparo não conseguiu rodar | veja o log; pode ser defeito físico |

## Para onde vão os arquivos danificados

Ao reparar, pedaços de arquivos que perderam o “endereço” não são apagados: são guardados como arquivos soltos.

- **FAT32 e exFAT:** em pastas como `FSCK0000.REC`, com arquivos `FILE0000.CHK`.
- **ext4:** na pasta `lost+found`, com nomes numéricos.

Às vezes dá para recuperar uma foto ou um documento abrindo esses arquivos; na maioria das vezes são restos que podem ser apagados.

## Quando reparar não basta

- **NTFS:** o `ntfsfix` usado no Linux só corrige o básico. Se restarem erros, conecte o disco num Windows e rode `chkdsk /f` na unidade.
- **Erros que voltam sempre:** o problema pode ser físico. Veja o tópico **Saúde** e rode a varredura de superfície: setores ilegíveis não se resolvem com reparo.
- **Arquivos que somem depois de cheio:** pode ser um pendrive falsificado (veja *Pendrives e cartões falsificados*).
- **“Não formatado” e o reparo não reconhece nada:** o índice pode estar destruído. Use um programa de recuperação de arquivos **antes** de formatar.

**Na prática:** faça **backup** do que ainda abre, verifique, repare se precisar e confira os arquivos importantes depois.

**Cuidado:** reparar mexe na estrutura do disco. Se os dados forem importantes e o disco estiver falhando fisicamente, copie o que der primeiro (ou faça uma imagem do disco) antes de qualquer reparo.
