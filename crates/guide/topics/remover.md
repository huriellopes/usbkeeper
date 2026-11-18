# Remover com segurança

Por que arrancar o pendrive pode corromper arquivos, o que significa estar em uso e como remover do jeito certo.

## Por que não dá para simplesmente tirar

Quando você copia um arquivo para o pendrive, o sistema muitas vezes **ainda não gravou tudo** quando a barra de cópia termina. Para ser rápido, ele guarda as gravações num **cache** na memória e as escreve no dispositivo aos poucos, segundos ou até minutos depois.

Se você tira o pendrive nesse intervalo:

- o arquivo copiado fica **incompleto ou corrompido**;
- a estrutura do sistema de arquivos pode ficar inconsistente, e o pendrive passa a pedir “reparo” ou aparece como “não formatado”;
- em casos piores, outros arquivos que já estavam lá também são afetados.

**Remover com segurança** força o sistema a terminar todas as gravações pendentes, desmonta o dispositivo (ninguém mais pode mexer nele) e, quando o hardware permite, **desliga** o dispositivo. Só então é seguro tirar.

## Como fazer no usbkeeper

1. Clique em **⏏ Remover com segurança**, no painel do dispositivo ou ao lado dele na lista.
2. Se der certo, aparece **“Pode tirar”** e o dispositivo some da lista.
3. Se aparecer **“desmontado, pode tirar quando o LED parar de piscar”**, o dispositivo não aceita ser desligado por software (comum em leitores de cartão). As gravações já foram concluídas; espere o **LED** (a luzinha) parar e tire.

Na linha de comando: `usbkeeper remover sdb`.

## Quando aparece “em uso”

“Em uso” quer dizer que algum programa está com um arquivo ou uma pasta do dispositivo aberta. Enquanto isso, o sistema não deixa desmontar, justamente para não corromper o que esse programa está fazendo.

O usbkeeper mostra **quais programas** estão usando o dispositivo e o que cada um tem aberto. Os culpados mais comuns são:

- uma janela do **gerenciador de arquivos** (Dolphin, Nautilus, Explorer) aberta numa pasta do pendrive;
- um **terminal** cujo diretório atual está dentro do pendrive (`cd /media/...`);
- um player de vídeo ou música tocando um arquivo dele;
- um editor de texto ou planilha com um documento dele aberto;
- uma cópia ainda em andamento.

Feche esses programas (ou, no terminal, saia da pasta com `cd`) e clique em **Tentar de novo**.

## E se eu já tirei sem remover?

Na maioria das vezes nada acontece. Mas, se o pendrive passou a dar erro:

- conecte de novo e deixe o sistema verificar/reparar (no Windows, “Verificar e reparar”; no Linux, `fsck`);
- abra os arquivos copiados por último e confira se estão inteiros;
- se o problema continuar, faça backup do que conseguir e formate.

**Na prática:** crie o hábito de sempre remover com segurança, principalmente depois de copiar arquivos grandes.

**Cuidado:** a luz do pendrive piscando quer dizer que ele ainda está trabalhando. Nunca tire com o LED piscando.
