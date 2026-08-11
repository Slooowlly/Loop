@echo off
rem ---------------------------------------------------------------------------
rem  Loop - teste do CARRO DESTRUIDO NA CLASSIFICACAO
rem
rem  Clique duas vezes neste arquivo. Ele abre o app com a regra experimental
rem  armada e uma janela separada acompanhando o log, que e onde o resultado do
rem  teste aparece.
rem
rem  A regra: se o carro voltar destruido da classificacao (reparo obrigatorio
rem  de 25s ou mais), o jogador larga do fundo (!eol); de 60s ou mais, nao larga
rem  (!dq). Fora deste atalho a regra fica desarmada.
rem
rem  Este arquivo e so para o teste. Pode apagar depois.
rem ---------------------------------------------------------------------------

title Loop - teste do carro destruido na classificacao
cd /d "%~dp0"

set IRACER_QUALI_WRECK=1

rem Log em janela propria. -Encoding UTF8 porque o log e UTF-8 e o PowerShell 5.1
rem le em ANSI por padrao, o que embaralha os acentos.
start "Log do Loop" powershell -NoExit -Command "Get-Content '%APPDATA%\com.loop.app\logs\loop.log' -Tail 5 -Wait -Encoding UTF8"

echo.
echo  Regra do carro destruido na classificacao: ARMADA
echo  Log: %APPDATA%\com.loop.app\logs\loop.log
echo.
echo  No fim da classificacao, procure no log:
echo    [iracing] carro da quali: reparo obrigatorio NNs, batida X -^> castigo Y
echo    [iracing] castigo da quali enviado: !eol #NN
echo.

rem --no-watch: sem isso, qualquer gravacao em src-tauri derruba o app no meio da
rem quali para recompilar. O Vite continua com HMR; so o Rust exige restart manual.
npm run tauri dev -- --no-watch
