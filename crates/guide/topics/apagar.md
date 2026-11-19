# Apagar com segurança

Antes de vender, doar ou descartar um pendrive ou HD, saiba o que realmente apaga os dados e o que só parece apagar.

## Por que formatar não basta

Formatar cria um índice novo e vazio, mas os dados antigos continuam lá (veja *O que formatar faz*). Qualquer programa de recuperação gratuito consegue trazer de volta fotos e documentos de um disco só formatado. Para entregar um disco a outra pessoa, é preciso **sobrescrever** os dados.

## HD × memória flash

- **HD (prato magnético):** cada endereço é um lugar fixo no prato. Sobrescrever o disco inteiro uma vez apaga de verdade; em HDs modernos, **uma passada basta**.
- **Pendrive, cartão e SSD (memória flash):** o controlador espalha as gravações e mantém **áreas de reserva** que o computador não enxerga (para trocar células gastas). Sobrescrever todos os endereços visíveis apaga quase tudo, mas **não garante** que cópias antigas nessas áreas de reserva sumiram. Com equipamento de laboratório, alguém poderia lê-las.

## Os métodos do usbkeeper

| Método | O que faz | Quando usar |
|---|---|---|
| Sobrescrever com zeros | grava zeros em todo o disco | padrão; HD e uso geral |
| Sobrescrever com dados aleatórios | grava bytes pseudoaleatórios | quando você não quer deixar um padrão reconhecível |
| Mais de uma passada | repete a sobrescrita (até 3) | exigência de alguma política; em HD moderno não acrescenta segurança |
| Verificar | relê tudo e confere que só restou o que foi gravado | para ter a prova de que gravou (e achar setores que não aceitam gravação) |
| Descartar (TRIM) | avisa o SSD que todos os blocos estão livres | SSD/NVMe: muito rápido; muitas pontes USB e pendrives não aceitam |

Sobrescrever o disco inteiro leva o tempo de gravar o disco todo: num pendrive comum, de minutos a algumas horas. A verificação leva mais ou menos o mesmo tempo de novo.

## Por que não “secure erase” de firmware

Alguns discos têm um comando próprio de apagamento total (ATA Security Erase, NVMe Format/Sanitize). Ele é ótimo quando funciona, mas: se for interrompido, pode **travar ou inutilizar** o disco; as pontes USB costumam bloquear o comando; e vários firmwares implementam mal. Por isso o usbkeeper não usa o *secure erase*.

## A garantia real: criptografia desde o início

Se o disco guardou dados sensíveis o tempo todo **criptografados** (LUKS no Linux, BitLocker no Windows, FileVault no Mac), basta destruir a chave: o que sobrar nas áreas de reserva é ilegível. Para dados que realmente importam, criptografe antes de usar o disco, não só antes de descartá-lo.

## Checklist antes de vender ou doar

1. Copie o que você quer guardar.
2. Confira que é o dispositivo certo (modelo, tamanho, série).
3. Apague: zeros, uma passada e **verificar** (ou descartar, num SSD que aceite).
4. Formate para entregar o disco pronto para uso.
5. Se o disco guardou algo muito sensível e não era criptografado, considere destruí-lo fisicamente.

**Na prática:** para a maioria das pessoas, sobrescrever com zeros em uma passada e verificar é o suficiente.

**Cuidado:** apagar é irreversível e começa a destruir os dados no primeiro segundo. Confira duas vezes o dispositivo escolhido e não interrompa no meio.
