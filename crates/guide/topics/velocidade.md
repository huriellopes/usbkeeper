# Velocidade: sequencial × 4K

Por que um pendrive copia um filme rápido mas trava abrindo muitos arquivos pequenos, e o que os números do teste querem dizer.

## Dois tipos de velocidade

O teste de velocidade mede duas coisas diferentes:

- **Sequencial (MB/s):** ler ou gravar um bloco grande e contínuo, como copiar um filme. É o número que aparece na embalagem.
- **Aleatória 4K (IOPS):** ler ou gravar muitos pedaços pequenos (4 KiB) espalhados pelo disco, como abrir um programa, rodar um sistema operacional ou copiar uma pasta com milhares de fotos pequenas. **IOPS** quer dizer “operações por segundo”.

Um pendrive pode fazer 100 MB/s sequenciais e só umas dezenas de IOPS em escrita 4K. Na prática, copiar um arquivo de 2 GB é rápido, mas copiar 2 GB de arquivos pequenos parece infinito.

## Faixas típicas

| Dispositivo | Leitura sequencial | Escrita 4K |
|---|---|---|
| Pendrive barato (USB 2.0) | 15 a 35 MB/s | poucas dezenas de IOPS |
| Pendrive bom (USB 3) | 100 a 400 MB/s | centenas a milhares de IOPS |
| HD externo | 80 a 150 MB/s | ~100 IOPS (limitado pela cabeça que se move) |
| SSD externo | 400 a 1000 MB/s | milhares de IOPS |
| NVMe em gaveta USB 3.2 Gen 2 | ~1000 MB/s | dezenas de milhares de IOPS |

Se a sua leitura bate no teto do link USB (~40 MB/s em USB 2.0, ~450 MB/s em USB 3.2 Gen 1), quem limita é o USB, não o dispositivo. O usbkeeper avisa quando isso acontece.

## O efeito do cache

Muitos pendrives e SSDs têm um **cache** rápido (às vezes chamado de cache SLC): uma área que aceita gravações muito rápido no começo. Quando ela enche, a velocidade despenca para a velocidade real dos chips, às vezes para um décimo. É por isso que uma cópia grande começa rápida e “trava” no meio. O teste do usbkeeper grava ~256 MB, o que mostra bem o começo; em cópias de dezenas de GB, espere velocidades menores.

## Teste só de leitura × com escrita

- **Medir leitura** não grava nada e pode ser feito a qualquer momento.
- **Medir leitura e escrita** grava dados de teste em partes do disco e **apaga o que estiver lá**. Exige confirmação.

**Na prática:** para rodar programas ou um sistema a partir do pendrive, olhe a escrita 4K; para guardar filmes e backups, olhe a sequencial.

**Cuidado:** um resultado muito abaixo do esperado pode ser porta, cabo ou hub limitando (veja o tópico *USB, cabos e velocidade*), e não o dispositivo.
