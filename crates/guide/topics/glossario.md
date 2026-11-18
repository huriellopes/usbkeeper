# Glossário

Os termos técnicos usados no usbkeeper, explicados em uma ou duas frases.

**Bloco**
: Unidade de leitura e gravação usada pelos testes do usbkeeper, normalmente de 4 KiB (8 setores).

**Cache**
: Memória rápida onde o sistema ou o próprio dispositivo guarda gravações temporariamente antes de escrevê-las de vez. É por causa dele que é preciso remover com segurança.

**Controlador**
: O pequeno processador dentro do pendrive, cartão ou SSD que decide onde cada dado fica fisicamente.

**Firmware**
: O programa interno do controlador. Nos pendrives falsificados, é ele que é alterado para anunciar uma capacidade que não existe.

**Flash NAND**
: O tipo de memória usado em pendrives, cartões e SSDs: guarda dados sem energia e se desgasta com o número de gravações.

**IOPS**
: Operações de entrada e saída por segundo. Mede quantos pedaços pequenos o dispositivo lê ou grava por segundo; importa para rodar programas.

**KiB, MiB, GiB × KB, MB, GB**
: KiB/MiB/GiB são potências de 2 (1 GiB = 1.073.741.824 bytes); KB/MB/GB são potências de 10 (1 GB = 1.000.000.000 bytes). Embalagens usam GB, por isso um “64 GB” aparece como ~59,6 GiB em alguns programas.

**LBA**
: Endereço lógico de um setor (o “número da página”). O sistema pede o setor pelo LBA e o controlador decide onde ele está de verdade.

**Montar**
: Ligar o sistema de arquivos de uma partição a uma pasta do sistema (como `/media/usuario/PENDRIVE`) para poder abrir os arquivos. Desmontar é o contrário, e é exigido antes de formatar ou remover.

**Partição**
: Uma fatia do disco, com começo e fim definidos na tabela de partições. Cada partição tem seu próprio sistema de arquivos.

**Ponte USB**
: O chip de uma gaveta ou adaptador que traduz USB para SATA ou NVMe. Define a velocidade máxima e se o SMART aparece.

**Rótulo**
: O nome do volume que aparece no gerenciador de arquivos (por exemplo, “FOTOS”).

**Setor**
: A menor unidade que o disco endereça, normalmente de 512 bytes (ou 4096 bytes em discos mais novos).

**SMART**
: Registro de saúde mantido pelo próprio HD ou SSD: setores defeituosos, temperatura, horas de uso, erros.

**Sistema de arquivos**
: A organização de pastas e arquivos dentro de uma partição (FAT32, exFAT, NTFS, ext4…).

**Tabela de partições**
: O mapa no começo do disco que diz onde cada partição começa e termina (MBR ou GPT).

**TRIM**
: Aviso que o sistema manda para um SSD dizendo quais blocos não estão mais em uso. Ajuda o SSD a manter a velocidade e a vida útil; muitas pontes USB não repassam.

**Velocidade negociada**
: A versão do USB que o dispositivo e a porta combinaram ao conectar. Um dispositivo USB 3 numa porta USB 2.0 negocia USB 2.0 e fica mais lento.

**Wipefs**
: Ferramenta que apaga as “assinaturas” de formatos antigos antes de formatar, para nenhum programa confundir o disco novo com restos do formato anterior.
