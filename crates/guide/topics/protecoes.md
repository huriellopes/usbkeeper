# O que o usbkeeper protege

As travas que impedem apagar o disco errado, e o que continua sendo responsabilidade sua.

## As travas

Antes de qualquer operação que grava no disco (formatar, teste completo de falsificação, teste de velocidade com escrita), o usbkeeper confere:

1. **O disco do sistema nunca é alvo.** Qualquer disco que tenha a raiz do sistema (`/`), a partição de boot, a `/home` ou a memória de troca (swap) montada é recusado, inclusive quando o sistema está dentro de criptografia ou LVM. Não existe opção para desligar essa trava.
2. **Só dispositivos USB, por padrão.** Discos internos SATA ou NVMe são recusados. Na linha de comando dá para liberar com `--allow-non-usb`, para quem sabe o que está fazendo; o disco do sistema continua proibido mesmo assim.
3. **Desmontado antes.** Se o dispositivo estiver montado, ele é desmontado antes; se algum programa estiver usando, a operação para e mostra qual.
4. **Confirmação pela série.** Você precisa digitar o número de série do dispositivo (ou o caminho, se ele não tiver série). Digitar o nome errado não apaga o disco errado.
5. **Reconferência da identidade.** Logo antes de gravar, o usbkeeper lê de novo o dispositivo e confere se a série e o tamanho continuam os mesmos. Se você trocou o pendrive enquanto confirmava, nada é gravado.
6. **Acesso exclusivo (O_EXCL).** O disco é aberto em modo exclusivo: se o sistema ainda tiver alguma partição dele montada, o próprio kernel recusa.
7. **O teste rápido restaura.** Antes de gravar seus blocos de teste, o teste rápido de falsificação salva o conteúdo original e o **restaura** no fim, inclusive se você cancelar no meio.
8. **Senha por operação.** A interface gráfica nunca roda como administrador; cada operação que acessa o disco passa pela senha do sistema e pela linha de comando, onde estão todas essas travas.

## O que continua sendo com você

- **Backup.** Nenhuma trava recupera um arquivo que só existia no pendrive que você decidiu formatar.
- **Escolher o dispositivo certo entre dois iguais.** Se dois pendrives idênticos estiverem conectados, a série é a única diferença. Confira ou desconecte um deles.
- **Ler os avisos.** Cada operação diz o que vai apagar e quanto deve demorar.
- **Não tirar o cabo no meio.** Use cancelar e espere.

## Histórico

Cada teste fica registrado no histórico do dispositivo (identificado por fabricante, modelo e série). Isso ajuda a acompanhar a saúde ao longo do tempo e serve de prova num pedido de reembolso.

**Na prática:** confie nas travas para evitar acidentes, mas não as use como substituto do backup.

**Cuidado:** pendrives baratos às vezes repetem o mesmo número de série entre unidades. Por isso o usbkeeper identifica o dispositivo também pelo fabricante, pelo modelo e pelo tamanho.
