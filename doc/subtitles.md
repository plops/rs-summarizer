yt-dlp supports subtitle/caption downloads for hundreds of websites beyond YouTube. The subtitle functionality is implemented through individual extractors that provide site-specific subtitle extraction methods.

## General Subtitle Support

The core subtitle processing is handled by `YoutubeDL.process_subtitles()` which manages subtitle selection, format preferences, and language filtering [1](#0-0) . The system supports both manual subtitles and automatic captions.

## Websites with Subtitle Support

Based on the extractor implementations, here are key websites that support subtitle downloads:

### Major Video Platforms
- **Bilibili**: Supports both danmaku (bullet comments) and regular subtitles in SRT format [2](#0-1) 
- **TikTok**: Extracts captions from multiple sources including creator captions, SRT, and WebVTT formats [3](#0-2) 
- **Facebook**: Supports both manual subtitles and automatic captions with locale-based language detection [4](#0-3) 
- **Vimeo**: Tested subtitle support for multiple languages including English, French, Spanish, and German [5](#0-4) 

### Educational and Corporate Platforms
- **Google Drive**: Extracts subtitles from video files with multiple format support [6](#0-5) 
- **Microsoft Medius**: Supports extensive subtitle collections with up to 30+ language tracks [7](#0-6) 
- **Panopto**: Converts JSON caption data to SRT format with multiple language support [8](#0-7) 
- **PeerTube**: Extracts captions through API endpoints with language ID mapping [9](#0-8) 

### Broadcasting and Media Sites
- **PBS**: Merges captions from multiple sources including HLS streams [10](#0-9) 
- **RTVE**: Spanish broadcaster with JSON-based subtitle extraction [11](#0-10) 
- **Rutube**: Russian platform with HLS subtitle extraction and caption merging [12](#0-11) 

### Additional Platforms
- **Naver**: Korean platform supporting both manual and automatic captions with fan-contributed subtitles [13](#0-12) 
- **Loom**: Video messaging platform with transcript extraction [14](#0-13) 
- **Dailymotion**: Tested with 6+ language subtitle support [15](#0-14) 

## Complete List

The full list of supported websites is available in `supportedsites.md` which contains over 1,800 extractors [16](#0-15) . While not all extractors implement subtitle support, a significant portion do, especially major video platforms and educational sites.

## Usage

To download subtitles, use options like:
- `--write-subs` for manual subtitles
- `--write-auto-subs` for automatic captions  
- `--sub-langs` to specify languages
- `--list-subs` to see available subtitles

The subtitle system supports multiple formats (SRT, VTT, ASS, TTML) and can handle both direct subtitle URLs and embedded caption data within video streams.

## Notes

- Subtitle availability varies by video and platform
- Some sites require login for subtitle access
- Automatic captions may have lower quality than manual subtitles
- The generic extractor may find subtitles on unsupported sites [17](#0-16) 

Wiki pages you might want to explore:
- [Overview (yt-dlp/yt-dlp)](/wiki/yt-dlp/yt-dlp#1)
- [Information Extraction Pipeline (yt-dlp/yt-dlp)](/wiki/yt-dlp/yt-dlp#2.2)
- [Glossary (yt-dlp/yt-dlp)](/wiki/yt-dlp/yt-dlp#9)
