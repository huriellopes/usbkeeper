# Primeiros passos e regras de ouro

O que o usbkeeper faz, o que pode dar errado e as regras que evitam perder dados.

## O que o usbkeeper faz

O usbkeeper cuida de pendrives, cartões de memória e HDs ou SSDs externos. Com ele você pode:

- **ver** cada dispositivo conectado: modelo, tamanho, número de série e a velocidade USB que ele conseguiu negociar;
- **descobrir se é falsificado**, ou seja, se a capacidade anunciada na embalagem existe de verdade;
- **conferir a saúde** (SMART, quando existe, e uma leitura completa da superfície);
- **medir a velocidade** de leitura e escrita;
- **formatar** do jeito certo para onde você vai usar;
- **remover com segurança**, sem corromper arquivos;
- guardar um **histórico** e gerar um **laudo** de cada dispositivo.

Algumas dessas operações **apagam dados**: formatar, o teste completo de falsificação e o teste de velocidade com escrita. Elas são marcadas como destrutivas e sempre pedem confirmação.

## Regras de ouro

1. **Faça backup antes.** Antes de qualquer operação destrutiva, copie para outro lugar tudo o que importa. Nenhum programa devolve um arquivo apagado com garantia.
2. **Confira se é o dispositivo certo.** Olhe o modelo, o tamanho e o número de série. Com dois pendrives iguais conectados, a série é a única coisa que diferencia um do outro.
3. **O disco do sistema nunca é alvo.** O disco onde o seu sistema está instalado aparece com um cadeado e não oferece ações destrutivas. Isso não tem como ser desligado.
4. **Não desconecte no meio de uma operação.** Tirar o pendrive durante uma formatação ou um teste deixa o dispositivo num estado incompleto. Use o botão de cancelar e espere ele terminar.
5. **Leia o aviso de cada operação.** Ele diz o que vai acontecer, o que será apagado e quanto tempo deve levar.
6. **Remova com segurança.** Antes de tirar o cabo, use “Remover com segurança” (veja o tópico *Remover com segurança*).

## Por que pede senha

Ler e gravar diretamente num disco é coisa de administrador no Linux. Por isso, testar, formatar e medir velocidade pedem a sua **senha**. A interface gráfica nunca roda como administrador: ela chama a linha de comando do usbkeeper só para aquela operação, e todas as travas de segurança ficam nessa linha de comando.

Depois da senha, o sistema lembra dela por alguns minutos. Assim, testar e logo em seguida formatar o mesmo pendrive não pede a senha duas vezes.

## A confirmação pela série

Para operações destrutivas, o usbkeeper pede que você **digite o número de série** do dispositivo (ou o caminho dele, como `/dev/sdb`, quando ele não tem série). Isso parece chato, mas evita o erro mais caro de todos: apagar o disco errado porque o nome mudou entre um clique e outro. Depois que você confirma, o usbkeeper ainda confere de novo se o dispositivo continua sendo o mesmo antes de gravar qualquer coisa.

**Na prática:** conecte só o dispositivo que você quer mexer, confira modelo e tamanho, faça backup e só então confirme.

**Cuidado:** se dois pendrives iguais estiverem conectados, desconecte um deles antes de formatar.
