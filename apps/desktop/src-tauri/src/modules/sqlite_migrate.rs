//! Versionamento de schema dos bancos SQLite do app — `PRAGMA user_version` como contador.
//!
//! Antes disto cada banco criava o schema com `CREATE ... IF NOT EXISTS` e engolia o erro de
//! `ALTER TABLE ADD COLUMN`: nada dizia em que versão um arquivo estava, e um app MAIS ANTIGO aberto
//! sobre um banco já migrado por um mais novo rodava às cegas (downgrade silencioso).
//!
//! [`migrate`] aplica `migrations[user_version..]` em ordem, cada uma em sua transação com o bump de
//! `user_version` dentro dela (falhou no meio = nada daquela migração fica). Reabrir é no-op, e
//! `user_version` maior que `migrations.len()` é recusado com erro claro em vez de seguir adiante.
//!
//! Banco legado (gravado antes do versionamento) tem `user_version = 0`: a migração 1 de cada banco
//! é o schema antigo inteiro (`CREATE ... IF NOT EXISTS` + os `ADD COLUMN` históricos) e, por isso,
//! tem de ser segura sobre tabelas que já existem. Para isso, um `ALTER TABLE ... ADD COLUMN` cuja
//! coluna já existe é tolerado (mesmo comportamento que `let _ = conn.execute(..)` tinha, agora
//! restrito a esse único erro); qualquer outro erro aborta a migração.
//!
//! Cada migração é um lote de statements separados por `;` — sem `;` dentro de literal.

use rusqlite::Connection;

/// Aplica as migrações pendentes. Idempotente.
pub fn migrate(conn: &Connection, migrations: &[&str]) -> Result<(), String> {
    let current = user_version(conn)?;
    let supported = migrations.len();
    if current > supported {
        return Err(format!(
            "o schema do banco é v{current}, mas esta versão do Mika só entende até v{supported} — \
             atualize o Mika (abrir assim arriscaria corromper os dados)"
        ));
    }
    for (index, batch) in migrations.iter().enumerate().skip(current) {
        apply_one(conn, batch, index + 1)?;
    }
    Ok(())
}

fn user_version(conn: &Connection) -> Result<usize, String> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("ler user_version: {e}"))?;
    Ok(usize::try_from(version).unwrap_or(0))
}

fn apply_one(conn: &Connection, batch: &str, target: usize) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| format!("migração v{target}: {e}"))?;
    for statement in batch.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        if let Err(e) = tx.execute_batch(statement) {
            if !is_existing_column(statement, &e) {
                return Err(format!("migração v{target}: {e}"));
            }
        }
    }
    tx.pragma_update(None, "user_version", target as i64).map_err(|e| format!("migração v{target}: user_version: {e}"))?;
    tx.commit().map_err(|e| format!("migração v{target}: commit: {e}"))
}

fn is_existing_column(statement: &str, error: &rusqlite::Error) -> bool {
    let upper = statement.to_ascii_uppercase();
    upper.starts_with("ALTER TABLE") && upper.contains("ADD COLUMN") && error.to_string().contains("duplicate column name")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(conn: &Connection) -> i64 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap()
    }

    fn has_column(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).unwrap();
        let names: Vec<String> = stmt.query_map([], |row| row.get(1)).unwrap().map(Result::unwrap).collect();
        names.iter().any(|n| n == column)
    }

    const M1: &str = "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, a TEXT); ALTER TABLE t ADD COLUMN b TEXT";
    const M2: &str = "CREATE TABLE u (id INTEGER PRIMARY KEY)";

    #[test]
    fn fresh_database_gets_every_migration_and_the_version() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, &[M1, M2]).unwrap();
        assert_eq!(version(&conn), 2);
        assert!(has_column(&conn, "t", "b"));
    }

    #[test]
    fn reopening_is_a_noop() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, &[M1, M2]).unwrap();
        conn.execute("INSERT INTO u (id) VALUES (7)", []).unwrap();
        migrate(&conn, &[M1, M2]).unwrap();
        assert_eq!(version(&conn), 2);
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM u", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    }

    /// Banco gravado antes do versionamento (`user_version = 0`), com e sem a coluna do `ALTER`:
    /// os dados sobrevivem e a migração 1 não falha por "duplicate column name".
    #[test]
    fn legacy_database_at_version_zero_migrates_without_loss() {
        for legacy_has_b in [false, true] {
            let conn = Connection::open_in_memory().unwrap();
            let ddl = if legacy_has_b { "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT, b TEXT)" } else { "CREATE TABLE t (id INTEGER PRIMARY KEY, a TEXT)" };
            conn.execute(ddl, []).unwrap();
            conn.execute("INSERT INTO t (id, a) VALUES (1, 'antigo')", []).unwrap();
            assert_eq!(version(&conn), 0);

            migrate(&conn, &[M1]).unwrap();

            assert_eq!(version(&conn), 1);
            assert!(has_column(&conn, "t", "b"));
            assert_eq!(conn.query_row("SELECT a FROM t WHERE id = 1", [], |r| r.get::<_, String>(0)).unwrap(), "antigo");
        }
    }

    #[test]
    fn newer_schema_than_supported_is_refused_with_a_clear_error() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 5).unwrap();
        let err = migrate(&conn, &[M1, M2]).unwrap_err();
        assert!(err.contains("v5") && err.contains("v2") && err.contains("atualize"), "{err}");
        assert_eq!(version(&conn), 5, "recusar não pode mexer no banco");
    }

    #[test]
    fn a_failing_migration_rolls_back_including_the_version() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, &[M1]).unwrap();
        let err = migrate(&conn, &[M1, "CREATE TABLE v (id INTEGER); CREATE TABLE t (id INTEGER)"]).unwrap_err();
        assert!(err.contains("migração v2"), "{err}");
        assert_eq!(version(&conn), 1);
        assert!(conn.prepare("SELECT 1 FROM v").is_err(), "a tabela da migração falha não pode sobrar");
    }
}
