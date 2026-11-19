# Imagem do disco e ISO de boot

Como fazer um backup do pendrive ou HD inteiro, restaurá-lo depois, e gravar uma ISO para criar um pendrive de boot sem erro.

## O que é uma imagem do disco

Uma **imagem** é a cópia de todos os bytes do dispositivo num arquivo: tabela de partições, sistemas de arquivos, arquivos e até o espaço vazio. Restaurar a imagem devolve o disco exatamente ao estado em que estava, inclusive se ele dava boot.

É o melhor **backup** antes de qualquer coisa arriscada: testar se é falsificado com o teste completo, reparar um sistema de arquivos com problemas, formatar ou apagar.

## Compressão

O usbkeeper comprime a imagem com **zstd**: o espaço vazio (que costuma ser zeros) quase não ocupa lugar, então um pendrive de 64 GB com 5 GB de arquivos vira um arquivo de poucos GB. A compressão é rápida e não perde nada. Se preferir um `.img` cru (que qualquer programa abre), crie sem compressão.

## Hash: a prova de que nada mudou

Junto da imagem, o usbkeeper grava um arquivo `.json` com o modelo, a série, o tamanho e o **hash SHA-256** dos dados. O hash é uma “impressão digital” de 64 caracteres: qualquer byte diferente muda o hash inteiro. Com ele dá para:

- **verificar** se o arquivo de backup continua íntegro meses depois;
- **conferir** depois de restaurar que o disco ficou idêntico ao original.

## Restaurar

Restaurar apaga tudo o que houver no disco de destino. O destino precisa ter o mesmo **tamanho** ou ser maior que o disco original; se for maior, o espaço a mais fica sem uso (dá para ampliar a partição depois com um gerenciador de partições). Depois de gravar, o usbkeeper relê o disco e compara o SHA-256.

## Gravar uma ISO de boot

Distribuições Linux e muitas ferramentas são distribuídas como **ISO**. Para dar boot por um pendrive, a ISO precisa ser **híbrida**: ter um setor de boot no começo, além do formato de CD. Quase todas as distribuições Linux atuais são híbridas; o usbkeeper avisa quando não é.

Antes de gravar, confira se o download não veio corrompido. O site da distribuição publica o SHA-256 (num arquivo `SHA256SUMS` ou na página de download). O usbkeeper confere sozinho se você passar o hash ou deixar o arquivo `.sha256`/`SHA256SUMS` na mesma pasta da ISO, e **recusa gravar** se o hash não bater. Depois de gravar, relê o pendrive e confere de novo.

## Por que a ISO do Windows é diferente

A ISO de instalação do **Windows** não é híbrida e tem um arquivo maior que 4 GB: gravada byte a byte ela **não dá boot**. Ela precisa de um preparo especial (partição FAT32 e o arquivo de instalação dividido), que o usbkeeper ainda não faz. Por isso ele reconhece essas ISOs pelo nome do volume e recusa gravá-las, explicando o motivo; para elas, use uma ferramenta própria de criar pendrive do Windows.

**Na prática:** antes de mexer num pendrive com dados importantes, crie uma imagem; para gravar uma ISO, deixe o `SHA256SUMS` ao lado dela.

**Cuidado:** gravar uma ISO apaga tudo o que havia no pendrive. Uma imagem guardada num disco que também falhou não serve de backup: guarde em outro lugar.
