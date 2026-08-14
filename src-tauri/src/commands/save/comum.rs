//! Utilitarios compartilhados do save: checkpoint do WAL, atualizacao dos carimbos
//! de tempo em meta.json, troca segura de arquivo/diretorio e parsing do
//! identificador da carreira.

use super::*;

pub(super) fn checkpoint_wal(db: &Database) -> Result<(), String> {
    crate::db::connection::checkpoint_wal_truncate(&db.conn)
        .map_err(|e| format!("Falha no WAL checkpoint: {e}"))
}

/// Le o meta.json, aplica a mutacao e devolve o JSON ja serializado, sem gravar.
///
/// Existe separado do `write_meta` porque o backup so pode carimbar sucesso em disco
/// depois que o arquivo final do backup realmente estiver no lugar.
pub(super) fn render_meta_timestamps<F>(meta_path: &Path, mutate: F) -> Result<String, String>
where
    F: FnOnce(&mut SaveMeta),
{
    let content =
        std::fs::read_to_string(meta_path).map_err(|e| format!("Falha ao ler meta.json: {e}"))?;
    let mut meta: SaveMeta =
        serde_json::from_str(&content).map_err(|e| format!("Falha ao parsear meta.json: {e}"))?;
    mutate(&mut meta);
    serde_json::to_string_pretty(&meta).map_err(|e| format!("Falha ao serializar meta.json: {e}"))
}

pub(super) fn write_meta(meta_path: &Path, payload: &str) -> Result<(), String> {
    std::fs::write(meta_path, payload).map_err(|e| format!("Falha ao gravar meta.json: {e}"))
}

pub(super) fn update_meta_timestamps<F>(meta_path: &Path, mutate: F) -> Result<(), String>
where
    F: FnOnce(&mut SaveMeta),
{
    let updated = render_meta_timestamps(meta_path, mutate)?;
    write_meta(meta_path, &updated)
}

/// Coloca `staged` no lugar de `destino` sem apagar o conteudo bom antes da hora.
///
/// A ordem ingenua (remover o destino e so entao renomear o staged) perde dado de
/// verdade: se o rename falha por disco cheio, permissao ou arquivo travado, o
/// backup bom ja foi apagado e nao volta. Aqui o antigo sai de cena por rename para
/// um irmao `.old`, que so e removido depois do novo estar efetivamente no lugar. Se
/// o rename final falhar, o `.old` e devolvido ao caminho original.
///
/// Serve para arquivo e para diretorio: a remocao escolhe a chamada certa pelo tipo
/// do caminho.
pub(crate) fn substituir_preservando_anterior(
    staged: &Path,
    destino: &Path,
    rotulo: &str,
) -> Result<(), String> {
    let anterior = caminho_anterior(destino);

    if anterior.exists() {
        remover_caminho(&anterior).map_err(|e| {
            format!(
                "Falha ao limpar {rotulo} anterior '{}': {e}",
                anterior.display()
            )
        })?;
    }

    let tinha_anterior = destino.exists();
    if tinha_anterior {
        std::fs::rename(destino, &anterior).map_err(|e| {
            format!(
                "Falha ao reservar {rotulo} anterior '{}' em '{}': {e}",
                destino.display(),
                anterior.display()
            )
        })?;
    }

    if let Err(erro_rename) = std::fs::rename(staged, destino) {
        let falha = format!(
            "Falha ao finalizar {rotulo} '{}' a partir de '{}': {erro_rename}",
            destino.display(),
            staged.display()
        );

        if tinha_anterior {
            if let Err(erro_rollback) = std::fs::rename(&anterior, destino) {
                return Err(format!(
                    "{falha}. Falha tambem ao devolver {rotulo} anterior de '{}': {erro_rollback}",
                    anterior.display()
                ));
            }
        }

        return Err(falha);
    }

    if tinha_anterior {
        // O novo ja esta no lugar. Sobra de `.old` nao invalida a troca, entao um erro
        // aqui nao pode derrubar um backup que deu certo: a proxima troca limpa.
        let _ = remover_caminho(&anterior);
    }

    Ok(())
}

/// Interruptor SO DE TESTE: faz a publicacao do item de indice `n` da troca em lote
/// falhar, para exercitar o rollback com metade dos itens ja publicados. `None` desliga.
/// Some inteiro do binario de release.
#[cfg(test)]
thread_local! {
    pub(crate) static SABOTAR_ITEM_DA_TROCA_EM_LOTE: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

/// Uma substituicao que so vale junto com as outras do lote.
struct ItemDeTroca {
    destino: PathBuf,
    /// `None` = o destino tem de deixar de existir depois da publicacao.
    staged: Option<PathBuf>,
    rotulo: String,
}

/// O que ja foi mexido, na ordem em que foi mexido, para o rollback saber desfazer.
struct ItemPublicado {
    destino: PathBuf,
    anterior: Option<PathBuf>,
}

/// Troca em lote: varias substituicoes que so fazem sentido juntas.
///
/// [`substituir_preservando_anterior`] protege UM caminho por vez. A restauracao dos
/// arquivos auxiliares mexe em varios, e copiar um por cima do outro deixava metade da
/// linha temporal antiga e metade da nova quando a copia do meio falhava — um estado que
/// o jogo abre sem reclamar e que ninguem consegue desfazer depois.
///
/// Aqui cada item e preparado num staging `.novo` ao lado do destino, sem encostar no
/// arquivo vivo. A publicacao move todos, e um erro no meio devolve ao lugar os que ja
/// tinham sido movidos. Serve para arquivo e para diretorio.
pub(crate) struct TrocaEmLote {
    itens: Vec<ItemDeTroca>,
}

impl TrocaEmLote {
    pub(crate) fn nova() -> Self {
        Self { itens: Vec::new() }
    }

    /// Copia `origem` para o staging do `destino`. Nada e publicado aqui.
    pub(crate) fn preparar_copia(
        &mut self,
        origem: &Path,
        destino: &Path,
        rotulo: &str,
    ) -> Result<(), String> {
        let staged = caminho_de_staging(destino);
        if staged.exists() {
            // Sobra de uma troca que morreu antes do `Drop` rodar (crash, queda de luz).
            remover_caminho(&staged).map_err(|e| {
                format!(
                    "Falha ao limpar {rotulo} em staging '{}': {e}",
                    staged.display()
                )
            })?;
        }

        copiar_caminho(origem, &staged).map_err(|e| {
            let _ = remover_caminho(&staged);
            format!(
                "Falha ao preparar {rotulo} a partir de '{}': {e}",
                origem.display()
            )
        })?;

        self.itens.push(ItemDeTroca {
            destino: destino.to_path_buf(),
            staged: Some(staged),
            rotulo: rotulo.to_string(),
        });
        Ok(())
    }

    /// Marca o `destino` para sumir na publicacao. O arquivo vivo continua intocado ate la.
    pub(crate) fn preparar_remocao(&mut self, destino: &Path, rotulo: &str) {
        self.itens.push(ItemDeTroca {
            destino: destino.to_path_buf(),
            staged: None,
            rotulo: rotulo.to_string(),
        });
    }

    /// Publica o lote inteiro. Em erro, desfaz o que ja tinha entrado e devolve a falha
    /// original junto com o que o rollback nao conseguiu desfazer.
    pub(crate) fn confirmar(mut self) -> Result<(), String> {
        let itens = std::mem::take(&mut self.itens);
        let mut feitos = Vec::new();

        for (indice, item) in itens.iter().enumerate() {
            if let Err(falha) = publicar_item(item, indice, &mut feitos) {
                let mut erros = vec![falha];
                desfazer_publicacao(&feitos, &mut erros);
                for restante in &itens {
                    if let Some(staged) = &restante.staged {
                        let _ = remover_caminho(staged);
                    }
                }
                return Err(erros.join(". "));
            }
        }

        for feito in &feitos {
            // O novo ja esta no lugar. Sobra de `.old` nao invalida a troca — a proxima
            // publicacao limpa.
            if let Some(anterior) = &feito.anterior {
                let _ = remover_caminho(anterior);
            }
        }
        Ok(())
    }
}

impl Drop for TrocaEmLote {
    fn drop(&mut self) {
        // Depois de `confirmar` a lista esta vazia e isto e no-op. Em qualquer outra
        // saida (erro, `?`, panic) e o que impede o staging de sobreviver a decisao.
        for item in &self.itens {
            if let Some(staged) = &item.staged {
                let _ = remover_caminho(staged);
            }
        }
    }
}

fn publicar_item(
    item: &ItemDeTroca,
    indice: usize,
    feitos: &mut Vec<ItemPublicado>,
) -> Result<(), String> {
    let anterior = caminho_anterior(&item.destino);
    if anterior.exists() {
        remover_caminho(&anterior).map_err(|e| {
            format!(
                "Falha ao limpar {} anterior '{}': {e}",
                item.rotulo,
                anterior.display()
            )
        })?;
    }

    let tinha_anterior = item.destino.exists();
    if tinha_anterior {
        std::fs::rename(&item.destino, &anterior).map_err(|e| {
            format!(
                "Falha ao reservar {} anterior '{}': {e}",
                item.rotulo,
                item.destino.display()
            )
        })?;
    }

    // Registrado ANTES da entrada do staging: a partir daqui o destino ja saiu de cena e
    // o rollback precisa saber devolve-lo, mesmo que o rename abaixo falhe.
    feitos.push(ItemPublicado {
        destino: item.destino.clone(),
        anterior: tinha_anterior.then(|| anterior.clone()),
    });

    #[cfg(test)]
    if SABOTAR_ITEM_DA_TROCA_EM_LOTE.with(|interruptor| interruptor.get()) == Some(indice) {
        return Err(format!("Falha injetada ao publicar {}", item.rotulo));
    }
    #[cfg(not(test))]
    let _ = indice;

    if let Some(staged) = &item.staged {
        std::fs::rename(staged, &item.destino).map_err(|e| {
            format!(
                "Falha ao finalizar {} '{}': {e}",
                item.rotulo,
                item.destino.display()
            )
        })?;
    }
    Ok(())
}

/// Devolve ao lugar tudo o que ja tinha sido publicado, do ultimo para o primeiro.
/// Continua mesmo com erro: parar no meio deixaria justamente o estado misto que a
/// troca em lote existe para evitar.
fn desfazer_publicacao(feitos: &[ItemPublicado], erros: &mut Vec<String>) {
    for feito in feitos.iter().rev() {
        if feito.destino.exists() {
            if let Err(e) = remover_caminho(&feito.destino) {
                erros.push(format!(
                    "Falha ao desfazer '{}': {e}",
                    feito.destino.display()
                ));
                continue;
            }
        }
        if let Some(anterior) = &feito.anterior {
            if let Err(e) = std::fs::rename(anterior, &feito.destino) {
                erros.push(format!(
                    "Falha ao devolver '{}' de '{}': {e}",
                    feito.destino.display(),
                    anterior.display()
                ));
            }
        }
    }
}

fn caminho_de_staging(destino: &Path) -> PathBuf {
    let mut nome = destino
        .file_name()
        .map(|nome| nome.to_os_string())
        .unwrap_or_default();
    nome.push(".novo");
    destino.with_file_name(nome)
}

/// Copia arquivo ou arvore de diretorio. O destino nunca existe quando esta funcao e
/// chamada (o staging e limpo antes), entao nao ha merge com conteudo velho — e e assim
/// que o diretorio de telas restaurado fica igual ao do snapshot, sem sobrevivente.
fn copiar_caminho(origem: &Path, destino: &Path) -> std::io::Result<()> {
    if !origem.is_dir() {
        std::fs::copy(origem, destino)?;
        return Ok(());
    }

    std::fs::create_dir_all(destino)?;
    for entrada in std::fs::read_dir(origem)? {
        let entrada = entrada?;
        copiar_caminho(&entrada.path(), &destino.join(entrada.file_name()))?;
    }
    Ok(())
}

fn caminho_anterior(destino: &Path) -> PathBuf {
    let mut nome = destino
        .file_name()
        .map(|nome| nome.to_os_string())
        .unwrap_or_default();
    nome.push(".old");
    destino.with_file_name(nome)
}

fn remover_caminho(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

pub(crate) fn parse_career_number(career_id: &str) -> Result<u32, String> {
    let s = career_id.trim_start_matches("career_");
    s.parse::<u32>()
        .map_err(|_| format!("career_id invalido: '{career_id}'"))
}
