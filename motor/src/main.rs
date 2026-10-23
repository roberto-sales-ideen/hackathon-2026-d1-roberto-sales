//! motor --entrada entrada.jsonl [--regras regras/regras.yaml] [--todas] > saida.txt

use motor::manifesto;
use motor::motor::Motor;
use std::io::{Read, Write};
use std::process::exit;

const BLOCO: usize = 256 * 1024;

fn main() {
    let mut entrada = None;
    let mut regras = "regras/regras.yaml".to_string();
    let mut todas = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--entrada" => entrada = args.next(),
            "--regras" => regras = args.next().unwrap_or_default(),
            "--todas" => todas = true,
            _ => falha(&format!("argumento desconhecido: {a}")),
        }
    }
    let Some(entrada) = entrada else {
        falha("uso: motor --entrada <arquivo.jsonl> [--regras <regras.yaml>] [--todas]")
    };
    let m = manifesto::carregar(&regras).unwrap_or_else(|e| falha(&format!("regras: {e}")));
    let mut motor = Motor::novo(&m);
    motor.todas = todas;
    let mut arq =
        std::fs::File::open(&entrada).unwrap_or_else(|e| falha(&format!("{entrada}: {e}")));
    let mut stdout = std::io::stdout().lock();
    // SAFETY: o arquivo de entrada não é alterado por ninguém enquanto o motor roda
    let r = match unsafe { memmap2::Mmap::map(&arq) } {
        Ok(mapa) => {
            let _ = mapa.advise(memmap2::Advice::Sequential);
            processar_mapa(&mapa, &mut motor, &mut stdout)
        }
        Err(_) => processar_blocos(&mut arq, &mut motor, &mut stdout),
    };
    if let Err(e) = r {
        falha(&format!("{entrada}: {e}"));
    }
}

/// Arquivo mapeado em memória: o sistema traz as páginas conforme a leitura avança, sem copiar
/// o arquivo para o heap. Mediu ~6% menos tempo que `read` no Docker.
fn processar_mapa(dados: &[u8], motor: &mut Motor, saida: &mut impl Write) -> std::io::Result<()> {
    let mut dados = dados.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(dados);
    let mut out = Vec::with_capacity(BLOCO);
    while let Some(p) = memchr::memchr(b'\n', dados) {
        motor.linha(&dados[..p], &mut out);
        dados = &dados[p + 1..];
        if out.len() >= BLOCO {
            saida.write_all(&out)?;
            out.clear();
        }
    }
    if !dados.is_empty() {
        motor.linha(dados, &mut out);
    }
    saida.write_all(&out)?;
    saida.flush()
}

/// Plano B, para entrada que não dá para mapear (pipe, /dev/stdin): lê em blocos de tamanho fixo;
/// o pedaço de linha que sobra no fim do bloco vai para o começo do buffer antes da próxima leitura.
fn processar_blocos(
    arq: &mut impl Read,
    motor: &mut Motor,
    saida: &mut impl Write,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; BLOCO];
    let mut out = Vec::with_capacity(BLOCO);
    let (mut ini, mut fim) = (0, 0);
    let mut primeira = true;
    loop {
        if fim == buf.len() {
            buf.resize(buf.len() * 2, 0);
        }
        let n = arq.read(&mut buf[fim..])?;
        if n == 0 {
            if ini < fim {
                motor.linha(&buf[ini..fim], &mut out);
            }
            break;
        }
        fim += n;
        if primeira && fim >= 3 {
            primeira = false;
            if buf.starts_with(b"\xEF\xBB\xBF") {
                ini = 3;
            }
        }
        while let Some(p) = memchr::memchr(b'\n', &buf[ini..fim]) {
            motor.linha(&buf[ini..ini + p], &mut out);
            ini += p + 1;
        }
        if out.len() >= BLOCO {
            saida.write_all(&out)?;
            out.clear();
        }
        buf.copy_within(ini..fim, 0);
        fim -= ini;
        ini = 0;
    }
    saida.write_all(&out)?;
    saida.flush()
}

fn falha(msg: &str) -> ! {
    eprintln!("motor: {msg}");
    exit(2)
}
