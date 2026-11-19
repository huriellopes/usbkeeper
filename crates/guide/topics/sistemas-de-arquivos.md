# Sistemas de arquivos: FAT32, exFAT, NTFS e ext4

Qual escolher para que o pendrive ou HD funcione onde você precisa: Windows, macOS, Linux, TV, carro ou videogame.

## O que é um sistema de arquivos

Dentro de cada partição existe um **sistema de arquivos**: a forma de organizar pastas, nomes, tamanhos e onde cada pedaço de cada arquivo está. É ele que decide quem consegue ler o seu pendrive e qual o maior arquivo que cabe nele.

## Compatibilidade

| | FAT32 | exFAT | NTFS | ext4 |
|---|---|---|---|---|
| Windows | lê e grava | lê e grava | lê e grava | não (precisa de programa extra) |
| macOS | lê e grava | lê e grava | só lê | não |
| Linux | lê e grava | lê e grava | lê e grava | lê e grava |
| TV, som, carro | quase sempre | às vezes | raramente | não |
| Videogames | a maioria | os mais novos | alguns | não |
| Maior arquivo | 4 GB | sem limite prático | sem limite prático | sem limite prático |
| Journaling | não | não | sim | sim |

## O limite de 4 GB do FAT32

No FAT32, nenhum arquivo pode passar de **4 GB** (exatamente 4 GiB menos 1 byte). Um filme em alta resolução, uma imagem ISO de sistema ou um backup grande simplesmente não cabem, mesmo com o pendrive vazio. A mensagem costuma ser algo como “arquivo grande demais para o destino”.

## O que é journaling

Um sistema com **journaling** anota o que vai fazer antes de fazer. Se a energia cair ou o cabo sair no meio de uma gravação, ele consegue se recuperar sozinho na próxima vez. Sem journaling (FAT32 e exFAT), uma interrupção pode deixar a estrutura inconsistente e exigir um reparo, com risco de perder o arquivo que estava sendo gravado.

## Qual escolher

- **Pendrive para usar em qualquer computador (Windows, macOS e Linux):** **exFAT**. É o melhor equilíbrio, sem o limite de 4 GB.
- **Pendrive para TV, som do carro, impressora ou aparelho antigo:** **FAT32**. É o que esses aparelhos entendem.
- **HD externo usado só com Windows, ou só Windows e Linux:** **NTFS**. Tem journaling, o que protege melhor um disco grande.
- **Disco usado só com Linux (backup de servidor, por exemplo):** **ext4**. Guarda permissões de arquivos do Linux.

O macOS usa seus próprios sistemas de arquivos (APFS e HFS+), que o usbkeeper não cria. Para um disco que vai circular entre Mac e PC, use **exFAT**.

**Na prática:** na dúvida, escolha **exFAT** para pendrives e **NTFS** para HDs externos grandes que só vão para PCs.

**Cuidado:** o rótulo (nome do volume) também tem limites: no FAT32 são até 11 caracteres, sem acentos e sempre em maiúsculas. O usbkeeper avisa enquanto você digita.
