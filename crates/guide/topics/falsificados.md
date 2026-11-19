# Pendrives e cartões falsificados

Como funciona a fraude de capacidade, como o usbkeeper a detecta e o que fazer se o seu for falso.

## Como a fraude funciona

Um pendrive falsificado tem um chip pequeno (digamos, 8 GB) com o **firmware alterado** para anunciar um tamanho bem maior (64 GB, 128 GB, até “2 TB”). O computador acredita, mostra o tamanho falso e deixa você gravar. O problema aparece depois, quando você tenta abrir os arquivos.

Existem dois tipos comuns:

- **O que dá a volta:** depois do fim real, o endereço volta para o começo. Gravar na posição “9 GB” de um falso de 8 GB grava, na verdade, por cima do 1 GB. Arquivos antigos são destruídos em silêncio.
- **O que descarta:** o que passa do limite real simplesmente some. A gravação parece funcionar, mas ao ler o arquivo vem zerado ou corrompido.

Em ambos, os primeiros arquivos funcionam normalmente. A perda só aparece quando o pendrive enche além da capacidade real, e aí já é tarde.

## Sinais de alerta

- capacidade muito alta por um preço muito baixo;
- marca desconhecida, embalagem genérica ou “original” por metade do preço;
- fotos e vídeos que abrem corrompidos depois de copiados;
- arquivos que somem ou aparecem com nomes estranhos.

## Como o usbkeeper testa

Cada bloco gravado no teste leva um **carimbo** com o próprio endereço. Ao ler de volta, um bloco pode estar certo, ter lixo ou ter o carimbo de **outro** endereço (prova de que os dois apontam para o mesmo lugar físico).

| | Teste rápido | Teste completo |
|---|---|---|
| Quanto demora | segundos | o tempo de gravar e ler o disco inteiro (horas em pendrives lentos) |
| Apaga dados? | não: salva e restaura os blocos usados | **sim, apaga tudo** |
| O que prova | pega os dois tipos comuns de falsificação | resposta definitiva e capacidade real exata |

Comece pelo **rápido**. Se ele disser “genuíno” e você ainda tiver dúvida, faça backup e rode o **completo**.

## Se o seu for falso

1. **Não guarde nada importante nele** além da capacidade real indicada.
2. **Guarde a prova:** salve o laudo (HTML na aba Histórico, ou `--json` na linha de comando).
3. **Peça reembolso:** lojas e marketplaces costumam aceitar o laudo como prova de produto falsificado.
4. **Se quiser aproveitar a parte real**, use “Reformatar só a parte real” (ou `format --max-size` na linha de comando). O usbkeeper cria uma partição que termina antes do limite real, com 2% de margem, sempre em MBR.

**Na prática:** teste todo pendrive ou cartão novo, principalmente os comprados pela internet, antes de colocar fotos ou documentos nele.

**Cuidado:** mesmo reformatado só na parte real, um pendrive falsificado costuma usar chips de baixa qualidade. Não use para a única cópia de algo importante.
