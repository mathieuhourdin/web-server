# OpenAI API Integration Setup

This project now includes OpenAI API integration for audio transcription and intelligent resource extraction using GPT-4, with optional web search enhancement.

## Environment Variables

Add the following environment variables to your `.env` file:

```bash
# OpenAI API Configuration
OPENAI_API_KEY=your_openai_api_key_here
OPENAI_API_BASE_URL=https://api.openai.com

# Optional: Web Search Enhancement (Google Custom Search API)
SEARCH_API_KEY=your_google_search_api_key_here
SEARCH_ENGINE_ID=your_custom_search_engine_id_here
```

## Getting API Keys

### OpenAI API Key
1. Go to [OpenAI Platform](https://platform.openai.com/)
2. Sign up or log in to your account
3. Navigate to "API Keys" in your dashboard
4. Create a new API key
5. Copy the key and add it to your `.env` file

### Google Custom Search API (Optional)
1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select existing one
3. Enable the "Custom Search API"
4. Create credentials (API key)
5. Go to [Google Programmable Search Engine](https://programmablesearchengine.google.com/)
6. Create a new search engine
7. Copy both the API key and search engine ID to your `.env` file

## Supported Audio Formats

The OpenAI Whisper API supports the following audio formats:
- mp3
- mp4
- mpeg
- mpga
- m4a
- wav
- webm

## API Endpoint

The audio processing endpoint is available at:
- `POST /process-audio` (multipart/form-data)

## Response Format

The API now returns structured JSON with resource information:

```json
{
  "message": "Audio file received, transcribed, and resource information extracted",
  "filename": "audio_file.mp3",
  "transcription": "This is the transcribed text from the audio file.",
  "resource_info": {
    "resource_title": "The Book Title",
    "author": "Author Name",
    "summary": "A comprehensive summary of the resource with key findings and significance...",
    "confidence": 0.95
  },
  "error": null
}
```

## Features

### 1. Audio Transcription
- Uses OpenAI Whisper API for high-quality transcription
- Supports multiple audio formats
- Handles various accents and languages

### 2. Intelligent Resource Extraction
- GPT-4 powered analysis of transcription
- Extracts book titles, author names, and academic references
- Provides confidence scores for extracted information
- Falls back to general content summary if no specific resource is mentioned

### 3. Web Search Enhancement (Optional)
- Automatically enhances summaries with web search results
- Only activates for high-confidence resource identifications
- Uses Google Custom Search API for relevant information
- Gracefully falls back if search API is unavailable

### 4. Structured JSON Response
- Consistent, parseable response format
- Confidence scoring for reliability assessment
- Comprehensive resource metadata

## Error Handling

The API provides comprehensive error handling for:
- Invalid audio files
- Transcription failures
- GPT API errors
- Search API failures (with graceful fallback)
- File upload issues

## Security Features

- Path traversal protection for uploaded filenames
- Temporary file storage in system temp directory
- Proper error handling and logging
- API key security through environment variables

## Performance Considerations

- GPT-4 model for highest accuracy in resource extraction
- Lower temperature (0.3) for consistent results
- Increased max tokens (800) for detailed responses
- Web search enhancement only for high-confidence extractions 