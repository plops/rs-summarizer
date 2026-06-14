// Supported models and their architectures
const MODELS = [
  { name: "gemini-3.5-flash", architecture: "Gemini" },
  { name: "gemma-4-31b-it", architecture: "Gemma" },
  { name: "gemma-4-26b-a4b-it", architecture: "Gemma" },
  { name: "gemini-3.1-flash-lite", architecture: "Gemini" },
  { name: "gemini-2.5-flash", architecture: "Gemini" },
  { name: "gemini-2.5-flash-lite", architecture: "Gemini" },
  { name: "gemini-3-flash-preview", architecture: "Gemini" }
];

document.addEventListener('DOMContentLoaded', async () => {
  const modelSelect = document.getElementById('modelSelect');
  const serverUrlInput = document.getElementById('serverUrlInput');
  const groundingCheckbox = document.getElementById('groundingCheckbox');
  const urlContextCheckbox = document.getElementById('urlContextCheckbox');
  
  const groundingContainer = document.getElementById('grounding-container');
  const urlContextContainer = document.getElementById('url-context-container');
  
  const summarizeBtn = document.getElementById('summarizeBtn');
  const resultContainer = document.getElementById('resultContainer');
  const resultBody = document.getElementById('resultBody');
  const statusLabel = document.getElementById('statusLabel');
  const pulseIndicator = document.getElementById('pulseIndicator');

  let pollIntervalId = null;

  // 1. Populate Models Dropdown
  MODELS.forEach(m => {
    const opt = document.createElement('option');
    opt.value = m.name;
    opt.textContent = m.name;
    modelSelect.appendChild(opt);
  });

  // 2. Load settings from storage
  const settings = await chrome.storage.local.get({
    serverUrl: 'https://rocketrecap.com',
    selectedModel: 'gemini-3.5-flash',
    grounding: false,
    urlContext: false
  });

  serverUrlInput.value = settings.serverUrl;
  modelSelect.value = settings.selectedModel;
  groundingCheckbox.checked = settings.grounding;
  urlContextCheckbox.checked = settings.urlContext;

  // 3. Update visibility of options depending on model
  function updateOptionsVisibility() {
    const selectedModelName = modelSelect.value;
    const model = MODELS.find(m => m.name === selectedModelName) || { architecture: 'Other' };

    if (model.architecture === 'Gemini') {
      groundingContainer.style.display = 'flex';
      urlContextContainer.style.display = 'flex';
    } else if (model.architecture === 'Gemma') {
      groundingContainer.style.display = 'flex';
      urlContextContainer.style.display = 'none';
      urlContextCheckbox.checked = false;
    } else {
      groundingContainer.style.display = 'none';
      urlContextContainer.style.display = 'none';
      groundingCheckbox.checked = false;
      urlContextCheckbox.checked = false;
    }
  }

  modelSelect.addEventListener('change', () => {
    updateOptionsVisibility();
    saveSettings();
  });
  serverUrlInput.addEventListener('change', saveSettings);
  groundingCheckbox.addEventListener('change', saveSettings);
  urlContextCheckbox.addEventListener('change', saveSettings);

  updateOptionsVisibility();

  async function saveSettings() {
    await chrome.storage.local.set({
      serverUrl: serverUrlInput.value.trim().replace(/\/$/, ''), // strip trailing slash
      selectedModel: modelSelect.value,
      grounding: groundingCheckbox.checked,
      urlContext: urlContextCheckbox.checked
    });
  }

  // 4. Summarize button click handler
  summarizeBtn.addEventListener('click', async () => {
    // Clear any active polling
    if (pollIntervalId) {
      clearInterval(pollIntervalId);
      pollIntervalId = null;
    }

    summarizeBtn.disabled = true;
    summarizeBtn.innerHTML = '<span class="spinner"></span> <span>Preparing...</span>';
    
    resultContainer.style.display = 'flex';
    statusLabel.textContent = 'Extracting Content';
    pulseIndicator.style.display = 'inline-block';
    resultBody.innerHTML = '<p>Reading active page content...</p>';

    try {
      const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
      const activeTab = tabs && tabs[0];
      if (!activeTab) {
        throw new Error("Could not retrieve active tab.");
      }

      // Inject content extraction script
      const injectionResults = await chrome.scripting.executeScript({
        target: { tabId: activeTab.id, allFrames: false },
        func: () => {
          const normalizeWhitespace = (text) => (text || '').replace(/\s+/g, ' ').trim();
          const getText = (selector) => {
            const element = document.querySelector(selector);
            return normalizeWhitespace(element ? element.innerText : '');
          };

          const selection = normalizeWhitespace(
            typeof window.getSelection === 'function' ? window.getSelection().toString() : ''
          );
          const bodyText = normalizeWhitespace(document.body ? document.body.innerText : '');
          const candidates = [
            getText('article'),
            getText('main'),
            getText('[role="main"]')
          ].filter(Boolean);
          const primaryText = candidates.reduce(
            (best, current) => (current.length > best.length ? current : best),
            ''
          );

          return {
            selection,
            text: primaryText.length >= 500 ? primaryText : bodyText
          };
        }
      });

      const pageResult = (injectionResults && injectionResults[0] && injectionResults[0].result) || {};
      const selectionText = pageResult.selection || '';
      const pageText = pageResult.text || '';
      const pageUrl = activeTab.url || '';

      // Determine what content to send
      let transcriptToSend = '';
      const isYouTube = pageUrl.includes('youtube.com/watch') || pageUrl.includes('youtu.be/');

      if (selectionText) {
        transcriptToSend = selectionText;
        statusLabel.textContent = 'Sending Selection';
        resultBody.innerHTML = '<p>Sending selected text to summarizer...</p>';
      } else if (isYouTube) {
        transcriptToSend = ''; // Server will use yt-dlp to download transcript
        statusLabel.textContent = 'Sending YouTube Link';
        resultBody.innerHTML = '<p>Sending YouTube video link to downloader...</p>';
      } else {
        transcriptToSend = pageText;
        statusLabel.textContent = 'Sending Page Text';
        resultBody.innerHTML = '<p>Sending page body text to summarizer...</p>';
      }

      // Build parameters
      const serverUrl = serverUrlInput.value.trim().replace(/\/$/, '');
      const selectedModel = modelSelect.value;
      const useGrounding = groundingCheckbox.checked && groundingContainer.style.display !== 'none';
      const useUrlContext = urlContextCheckbox.checked && urlContextContainer.style.display !== 'none';

      const formParams = new URLSearchParams();
      formParams.append('original_source_link', pageUrl);
      formParams.append('transcript', transcriptToSend);
      formParams.append('model', selectedModel);
      if (useGrounding) {
        formParams.append('google_search_grounding', 'true');
      }
      if (useUrlContext) {
        formParams.append('url_context', 'true');
      }

      // Send Request
      const response = await fetch(`${serverUrl}/process_transcript`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/x-www-form-urlencoded'
        },
        body: formParams.toString()
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(errorText || `HTTP error! Status: ${response.status}`);
      }

      const responseHtml = await response.text();
      resultBody.innerHTML = responseHtml;
      
      // Parse Response to find the Generation ID
      const generationIdMatch = responseHtml.match(/href="\/generations\/(\d+)"|hx-post="\/generations\/(\d+)"/);
      const generationId = generationIdMatch ? (generationIdMatch[1] || generationIdMatch[2]) : null;

      if (generationId) {
        statusLabel.textContent = 'Generating Summary';
        // Start polling the server
        startPolling(serverUrl, generationId);
      } else {
        statusLabel.textContent = 'Completed';
        pulseIndicator.style.display = 'none';
        summarizeBtn.disabled = false;
        summarizeBtn.innerHTML = '<span>Summarize Current Page</span>';
      }

    } catch (err) {
      console.error(err);
      statusLabel.textContent = 'Error';
      pulseIndicator.style.display = 'none';
      resultBody.innerHTML = `<p style="color:var(--error);">Failed: ${err.message}</p>`;
      summarizeBtn.disabled = false;
      summarizeBtn.innerHTML = '<span>Summarize Current Page</span>';
    }
  });

  // Polling function for real-time update
  function startPolling(serverUrl, generationId) {
    pollIntervalId = setInterval(async () => {
      try {
        const res = await fetch(`${serverUrl}/generations/${generationId}`, {
          method: 'POST'
        });

        if (!res.ok) {
          throw new Error(`Polling HTTP error: ${res.status}`);
        }

        const html = await res.text();
        resultBody.innerHTML = html;

        // Check if finished (completed response won't have hx-post or hx-get attribute anymore)
        const isDone = !html.includes('hx-post=') && !html.includes('hx-get=') && html.includes('Summary Complete');
        const hasError = html.includes('not found') || html.includes('Error:');

        if (isDone || hasError) {
          clearInterval(pollIntervalId);
          pollIntervalId = null;
          statusLabel.textContent = isDone ? 'Summary Complete' : 'Error';
          pulseIndicator.style.display = 'none';
          summarizeBtn.disabled = false;
          summarizeBtn.innerHTML = '<span>Summarize Current Page</span>';
        }
      } catch (err) {
        console.error("Polling error:", err);
        clearInterval(pollIntervalId);
        pollIntervalId = null;
        statusLabel.textContent = 'Polling Error';
        pulseIndicator.style.display = 'none';
        summarizeBtn.disabled = false;
        summarizeBtn.innerHTML = '<span>Summarize Current Page</span>';
      }
    }, 1500); // Poll every 1.5 seconds
  }
});
