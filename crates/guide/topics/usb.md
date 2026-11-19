# USB, cabos e velocidade

As versões do USB, por que a mesma unidade pode ser rápida numa porta e lenta em outra, e como resolver.

## As versões

O nome comercial do USB muda de tempos em tempos, o que confunde. O que importa é a velocidade:

| Nome | Também chamado de | Velocidade teórica | Na prática |
|---|---|---|---|
| USB 2.0 | Hi-Speed | 480 Mb/s | 35 a 42 MB/s |
| USB 3.2 Gen 1 | USB 3.0, USB 3.1 Gen 1 | 5 Gb/s | até ~450 MB/s |
| USB 3.2 Gen 2 | USB 3.1 Gen 2 | 10 Gb/s | até ~1000 MB/s |
| USB 3.2 Gen 2x2 | — | 20 Gb/s | até ~2000 MB/s |

Repare na diferença entre **Mb/s** (megabits) e **MB/s** (megabytes): um byte tem 8 bits, e parte da banda é gasta com controle. Por isso 480 Mb/s viram, na vida real, uns 40 MB/s.

## O dispositivo e a porta precisam concordar

A velocidade final é a **menor** entre o dispositivo, o cabo, o hub e a porta. Um pendrive USB 3 numa porta USB 2.0 funciona, mas fica limitado a ~40 MB/s.

O usbkeeper mostra a velocidade **negociada** e avisa quando um dispositivo USB 3 está rodando como USB 2.0. Quando isso acontece, as causas mais comuns são:

1. **A porta é USB 2.0.** Portas USB 3 costumam ter o plástico interno azul (às vezes vermelho ou verde) ou o símbolo “SS”. Portas USB-C podem ser de qualquer versão.
2. **O cabo é USB 2.0.** Muitos cabos USB-C, principalmente os que vêm com celular, só têm os fios do USB 2.0.
3. **O hub é USB 2.0** ou divide a banda entre muitos dispositivos.
4. **A porta da frente do gabinete** está ligada à placa-mãe por um cabo interno USB 2.0.
5. **Mau contato:** um conector gasto às vezes cai para USB 2.0 sem avisar.

**Na prática:** troque de porta (prefira as de trás do computador, direto na placa-mãe), depois o cabo, e por último tire o hub do caminho. Rode o teste de velocidade de novo para conferir.

## Energia

HDs externos de 2,5” são alimentados pela própria porta USB. Algumas portas, hubs sem fonte e notebooks em economia de energia não entregam corrente suficiente. Os sintomas são o HD que liga e desliga, que faz cliques ou que some no meio de uma cópia.

- Ligue direto no computador, sem hub, ou use um **hub com fonte**.
- Alguns HDs antigos vêm com um cabo em “Y”, que puxa energia de duas portas.
- HDs de 3,5” sempre precisam de fonte própria.

**Cuidado:** um HD que perde energia no meio de uma gravação pode corromper arquivos e, com o tempo, se danificar. Se o seu HD externo desconecta sozinho, resolva a alimentação antes de confiar dados a ele.
