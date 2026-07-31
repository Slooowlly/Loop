; Hooks do instalador NSIS.
;
; Ao desinstalar, remove o REGISTRO da nossa API layer do OpenXR. Sem isto, o valor
; ficaria em HKCU apontando pra um manifesto que não existe mais: o loader do OpenXR
; reclama no log dele a cada app de VR aberto e segue em frente. Não quebra nada, mas é
; sujeira nossa deixada na máquina do jogador.
;
; O app (re)escreve manifesto + valor a cada boot, então quem cuida da INSTALAÇÃO é o
; Rust (commands/vr_layer.rs); aqui só a limpeza, que é o único momento em que o app
; não tem como agir por conta própria.
;
; O nome do valor é o CAMINHO do manifesto, que vive no app data do usuário — o mesmo
; que o Rust monta. Se aquele caminho mudar lá, muda aqui.

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegValue HKCU "Software\Khronos\OpenXR\1\ApiLayers\Implicit" "$APPDATA\com.loop.app\XR_APILAYER_NOVA_iracer_overlay.json"
  Delete "$APPDATA\com.loop.app\XR_APILAYER_NOVA_iracer_overlay.json"
!macroend
