# O que formatar faz (e o que não faz)

Formatar cria um índice novo e vazio; entender isso evita perder dados e também evita achar que os dados sumiram de verdade.

## Formatar é recriar o índice

Quando você formata, o usbkeeper faz três coisas:

1. **Apaga as assinaturas antigas** (os “carimbos” que dizem qual sistema de arquivos e qual tabela havia ali), para nenhum programa se confundir com restos do formato anterior.
2. **Cria uma tabela de partições nova**, MBR ou GPT, com uma única partição (veja o tópico *MBR × GPT*).
3. **Cria um sistema de arquivos vazio** dentro dela, com o rótulo que você escolheu.

Formatar **não passa por todos os setores do disco**. Ele escreve só o **índice** novo, que diz “este disco está vazio”. Por isso é rápido, e por isso os dados antigos continuam lá fisicamente, só que sem nada apontando para eles.

## Os arquivos ainda podem ser recuperados

Como os dados antigos não foram sobrescritos, programas de recuperação (como o PhotoRec) muitas vezes conseguem **recuperar** fotos e documentos de um pendrive recém-formatado. Isso é ótimo quando você formatou o disco errado: **pare de usar o dispositivo imediatamente** e tente recuperar antes de gravar qualquer coisa nova.

## Formatar não é apagar com segurança

Pelo mesmo motivo, formatar **não serve para apagar com segurança** antes de vender ou doar um pendrive. Mesmo sobrescrever o disco inteiro não é garantia em memória flash: o controlador guarda áreas de sobra que o computador nem enxerga, e dados antigos podem ficar nelas.

Para descartar um dispositivo com dados sensíveis:

- prefira ter usado **criptografia** desde o começo (sem a senha, os dados são inúteis);
- ou destrua o dispositivo fisicamente.

## Checklist antes de formatar

1. **Backup:** tudo o que importa está copiado em outro lugar?
2. **Dispositivo certo:** modelo, tamanho e série batem com o que você quer formatar?
3. **Ninguém usando:** feche janelas e programas abertos no dispositivo. Se ele estiver montado, o usbkeeper desmonta antes, ou explica quem está usando.
4. **Formato certo:** escolha o sistema de arquivos pelo lugar onde ele vai ser usado (veja *Sistemas de arquivos*).
5. **Falsificado?** Se o pendrive é suspeito, rode o teste de falsificação antes de formatar e confiar dados a ele.

**Na prática:** o usbkeeper mostra o que será apagado e pede para você digitar a série. Leia essa lista com calma: é a última chance de desistir.

**Cuidado:** depois de formatar o disco errado, não grave nada nele. Cada arquivo novo pode cair por cima de um arquivo antigo que ainda era recuperável.
