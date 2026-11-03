//! UI languages. Text is written in English in the code; `tr` swaps it for the chosen language.
use std::sync::atomic::{AtomicUsize, Ordering};

pub const LANGS: [&str; 3] = ["English", "Português (Brasil)", "Español"];
static LANG: AtomicUsize = AtomicUsize::new(0);

pub fn set(i: usize) {
    LANG.store(i.min(LANGS.len() - 1), Ordering::Relaxed);
}

/// Windows display language: Portuguese -> 1, Spanish -> 2, anything else -> English.
pub fn system_default() -> usize {
    match unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3FF {
        0x16 => 1,
        0x0A => 2,
        _ => 0,
    }
}

/// Translate an English UI string; unknown text (file names, numbers) is returned as is.
pub fn tr(s: &str) -> &str {
    let lang = LANG.load(Ordering::Relaxed);
    if lang == 0 {
        return s;
    }
    T.iter().find(|r| r.0 == s).map(|r| if lang == 1 { r.1 } else { r.2 }).unwrap_or(s)
}

// (English, Português do Brasil, Español)
const T: &[(&str, &str, &str)] = &[
    // tabs and toolbar
    ("Menu", "Menu", "Menú"),
    ("Screen Recording", "Gravar tela", "Grabar pantalla"),
    ("Game Recording", "Gravar jogo", "Grabar juego"),
    ("Sound Recording", "Gravar áudio", "Grabar audio"),
    ("Help", "Ajuda", "Ayuda"),
    ("Record", "Gravar", "Grabar"),
    ("Stop", "Parar", "Detener"),
    ("Cancel", "Cancelar", "Cancelar"),
    ("Pause", "Pausar", "Pausa"),
    ("Resume", "Continuar", "Reanudar"),
    ("Capture", "Capturar", "Capturar"),
    ("Size", "Tamanho", "Tamaño"),
    ("Open", "Abrir", "Abrir"),
    ("Codecs", "Codecs", "Códecs"),
    ("Sound", "Som", "Sonido"),
    ("Options", "Opções", "Opciones"),
    // status
    ("PAUSED", "PAUSADO", "PAUSA"),
    ("READY", "PRONTO", "LISTO"),
    ("Idle", "Parado", "Inactivo"),
    ("Free:", "Livre:", "Libre:"),
    ("Active game window", "Janela do jogo ativa", "Ventana del juego activa"),
    ("Audio only", "Somente áudio", "Solo audio"),
    ("PC sound + mic", "Som do PC + mic", "Sonido del PC + mic"),
    ("PC sound", "Som do PC", "Sonido del PC"),
    ("Microphone", "Microfone", "Micrófono"),
    ("No sound", "Sem som", "Sin sonido"),
    // menus
    ("Options...", "Opções...", "Opciones..."),
    ("Open output folder", "Abrir pasta de saída", "Abrir carpeta de salida"),
    ("Exit", "Sair", "Salir"),
    ("About ClearShot", "Sobre o ClearShot", "Acerca de ClearShot"),
    ("Language", "Idioma", "Idioma"),
    ("Full screen - Monitor", "Tela cheia - Monitor", "Pantalla completa - Monitor"),
    ("Select a window (click it)", "Selecionar uma janela (clique nela)", "Seleccionar una ventana (haz clic en ella)"),
    ("Hide recording frame", "Ocultar moldura de gravação", "Ocultar marco de grabación"),
    ("Show recording frame", "Mostrar moldura de gravação", "Mostrar marco de grabación"),
    ("Sound file:", "Arquivo de áudio:", "Archivo de audio:"),
    ("(needs FFmpeg)", "(requer FFmpeg)", "(requiere FFmpeg)"),
    ("Animated GIF", "GIF animado", "GIF animado"),
    ("Built-in H.264", "H.264 integrado", "H.264 integrado"),
    ("GPU H.264 (NVENC/QSV/AMF)", "H.264 na GPU (NVENC/QSV/AMF)", "H.264 por GPU (NVENC/QSV/AMF)"),
    ("Record PC sound", "Gravar som do PC", "Grabar sonido del PC"),
    ("Record microphone", "Gravar microfone", "Grabar micrófono"),
    // messages
    ("Saved", "Salvo", "Guardado"),
    ("Captured", "Capturado", "Capturado"),
    ("Capture failed:", "Falha na captura:", "Error de captura:"),
    ("Recording...", "Gravando...", "Grabando..."),
    ("No active window to record.", "Nenhuma janela ativa para gravar.", "No hay ventana activa para grabar."),
    ("No active window to capture.", "Nenhuma janela ativa para capturar.", "No hay ventana activa para capturar."),
    ("Sound is off: turn on PC sound or the microphone first.", "O som está desligado: ative o som do PC ou o microfone primeiro.", "El sonido está apagado: activa el sonido del PC o el micrófono primero."),
    ("Click on the window you want to record.", "Clique na janela que você quer gravar.", "Haz clic en la ventana que quieres grabar."),
    ("ClearShot - screen, game and sound recorder. No ads, ever.", "ClearShot - gravador de tela, jogos e áudio. Sem anúncios, nunca.", "ClearShot - grabador de pantalla, juegos y audio. Sin anuncios, nunca."),
    ("Stop the recording before switching modes.", "Pare a gravação antes de trocar de modo.", "Detén la grabación antes de cambiar de modo."),
    ("Game mode cancelled.", "Modo jogo cancelado.", "Modo juego cancelado."),
    ("Game mode ready: bring your game to the front and press", "Modo jogo pronto: traga o jogo para a frente e pressione", "Modo juego listo: trae el juego al frente y pulsa"),
    ("to start / stop.", "para iniciar / parar.", "para iniciar / detener."),
    ("Ready. Drag the green frame to choose what to record.", "Pronto. Arraste a moldura verde para escolher o que gravar.", "Listo. Arrastra el marco verde para elegir qué grabar."),
    ("Frame fitted to the window", "Moldura ajustada à janela", "Marco ajustado a la ventana"),
    ("needs FFmpeg (ffmpeg.exe on PATH or next to this app); otherwise MP4 is saved.", "requer o FFmpeg (ffmpeg.exe no PATH ou ao lado deste app); senão, é salvo em MP4.", "requiere FFmpeg (ffmpeg.exe en el PATH o junto a esta app); si no, se guarda en MP4."),
    // options
    ("Saving", "Salvamento", "Guardado"),
    ("Save folder", "Pasta de destino", "Carpeta de destino"),
    ("Browse...", "Procurar...", "Examinar..."),
    ("Video", "Vídeo", "Vídeo"),
    ("File format", "Formato do arquivo", "Formato de archivo"),
    ("Encoder", "Codificador", "Codificador"),
    ("Animated GIF (no sound)", "GIF animado (sem som)", "GIF animado (sin sonido)"),
    ("Frame rate", "Quadros por segundo", "Fotogramas por segundo"),
    ("Quality (bitrate)", "Qualidade (bitrate)", "Calidad (bitrate)"),
    ("GIF frame rate", "Quadros do GIF", "Fotogramas del GIF"),
    ("Show the mouse pointer in recordings and captures", "Mostrar o ponteiro do mouse nas gravações e capturas", "Mostrar el puntero del ratón en grabaciones y capturas"),
    ("Recording frame", "Moldura de gravação", "Marco de grabación"),
    ("Frame opacity", "Opacidade da moldura", "Opacidad del marco"),
    ("Sound & images", "Som e imagens", "Sonido e imágenes"),
    ("Sound file format", "Formato do áudio", "Formato de audio"),
    ("Capture image format", "Formato da imagem", "Formato de imagen"),
    ("Hotkeys (work everywhere, also in games)", "Atalhos (funcionam em todo lugar, até em jogos)", "Atajos (funcionan en todas partes, también en juegos)"),
    ("Press a key...", "Pressione uma tecla...", "Pulsa una tecla..."),
    ("Record / Stop", "Gravar / Parar", "Grabar / Detener"),
    ("Pause / Resume", "Pausar / Continuar", "Pausa / Reanudar"),
    ("Watermark & webcam", "Marca d'água e webcam", "Marca de agua y webcam"),
    ("Watermark text", "Texto da marca d'água", "Texto de marca de agua"),
    ("Watermark image", "Imagem da marca d'água", "Imagen de marca de agua"),
    ("(none) - click to type", "Clique para digitar", "Clic para escribir"),
    ("(none)", "(nenhuma)", "(ninguna)"),
    ("Choose...", "Escolher...", "Elegir..."),
    ("Clear", "Limpar", "Borrar"),
    ("Webcam overlay", "Sobrepor webcam", "Superponer webcam"),
    ("needs FFmpeg", "requer FFmpeg", "requiere FFmpeg"),
    ("Done", "Concluir", "Listo"),
    ("Top left", "Sup. esquerdo", "Sup. izquierda"),
    ("Top right", "Sup. direito", "Sup. derecha"),
    ("Bottom left", "Inf. esquerdo", "Inf. izquierda"),
    ("Bottom right", "Inf. direito", "Inf. derecha"),
    // custom size
    ("Custom size...", "Tamanho personalizado...", "Tamaño personalizado..."),
    ("Custom size", "Tamanho personalizado", "Tamaño personalizado"),
    ("Width", "Largura", "Ancho"),
    ("Height", "Altura", "Alto"),
    ("OK", "OK", "Aceptar"),
    ("Frame size:", "Tamanho da moldura:", "Tamaño del marco:"),
    ("Size must be at least 64 x 64.", "O tamanho mínimo é 64 x 64.", "El tamaño mínimo es 64 x 64."),
    // file name
    ("File name", "Nome do arquivo", "Nombre de archivo"),
    ("Next number {n}", "Próximo número {n}", "Próximo número {n}"),
    ("Example:", "Exemplo:", "Ejemplo:"),
    // fps
    ("Show actual FPS while recording", "Mostrar o FPS real durante a gravação", "Mostrar los FPS reales durante la grabación"),
    ("actual", "real", "real"),
    // click effects
    ("Click effects", "Efeitos de clique", "Efectos de clic"),
    ("Show mouse clicks in recordings", "Mostrar os cliques do mouse nas gravações", "Mostrar los clics del ratón en las grabaciones"),
    ("Left / right click colour", "Cor do clique esq. / dir.", "Color del clic izq. / der."),
    ("Ring size / duration", "Tamanho / duração do anel", "Tamaño / duración del anillo"),
    ("Red", "Vermelho", "Rojo"),
    ("Blue", "Azul", "Azul"),
    ("Green", "Verde", "Verde"),
    ("Yellow", "Amarelo", "Amarillo"),
    ("White", "Branco", "Blanco"),
    // devices
    ("Windows default", "Padrão do Windows", "Predeterminado de Windows"),
    ("Webcam device", "Dispositivo da webcam", "Dispositivo de webcam"),
    ("Automatic (first found)", "Automático (o primeiro encontrado)", "Automático (el primero encontrado)"),
];

#[cfg(test)]
mod tests {
    #[test]
    fn no_duplicate_keys() {
        for (i, a) in super::T.iter().enumerate() {
            assert!(super::T[i + 1..].iter().all(|b| b.0 != a.0), "duplicate key {}", a.0);
        }
    }
}
