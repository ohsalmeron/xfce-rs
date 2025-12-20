// XFCE App Finder - Frontend Logic

const { invoke } = window.__TAURI__.core;

// DOM Elements
const searchInput = document.getElementById('search-input');
const appGrid = document.getElementById('app-grid');

// Debounce helper
function debounce(fn, delay) {
    let timeoutId;
    return (...args) => {
        clearTimeout(timeoutId);
        timeoutId = setTimeout(() => fn(...args), delay);
    };
}

// Icon theme paths to search for icons
const ICON_THEME_PATHS = [
    '/usr/share/icons/hicolor',
    '/usr/share/icons/Adwaita',
    '/usr/share/icons/gnome',
    '/usr/share/pixmaps'
];

// Try to find an icon path
function getIconPath(iconName) {
    if (!iconName) return null;

    // If it's already an absolute path
    if (iconName.startsWith('/')) {
        return iconName;
    }

    // Common icon sizes to try (prefer larger)
    const sizes = ['256x256', '128x128', '64x64', '48x48', 'scalable'];
    const categories = ['apps', 'applications'];
    const extensions = ['.svg', '.png', '.xpm'];

    // For now, just try the pixmaps path with common extensions
    for (const ext of extensions) {
        const path = `/usr/share/pixmaps/${iconName}${iconName.includes('.') ? '' : ext}`;
        return path;
    }

    return null;
}

// Create an app card element
function createAppCard(app) {
    const card = document.createElement('div');
    card.className = 'app-card';
    card.tabIndex = 0; // Make focusable
    card.title = app.comment || app.name;

    // Icon
    const iconDiv = document.createElement('div');
    iconDiv.className = 'app-icon';

    const iconPath = getIconPath(app.icon);
    if (iconPath) {
        const img = document.createElement('img');
        img.src = iconPath;
        img.alt = app.name;
        img.onerror = () => {
            // Fallback to letter if icon fails to load
            iconDiv.innerHTML = `<span class="fallback">${app.name.charAt(0)}</span>`;
        };
        iconDiv.appendChild(img);
    } else {
        iconDiv.innerHTML = `<span class="fallback">${app.name.charAt(0)}</span>`;
    }

    // Name
    const nameDiv = document.createElement('div');
    nameDiv.className = 'app-name';
    nameDiv.textContent = app.name;

    // Category (optional)
    let categoryDiv = null;
    if (app.categories && app.categories.length > 0) {
        categoryDiv = document.createElement('div');
        categoryDiv.className = 'app-category';
        // Show first meaningful category
        const category = app.categories.find(c => !['Application', 'GTK', 'Qt', 'GNOME', 'KDE', 'X-'].some(skip => c.includes(skip))) || app.categories[0];
        categoryDiv.textContent = category;
    }

    card.appendChild(iconDiv);
    card.appendChild(nameDiv);
    if (categoryDiv) card.appendChild(categoryDiv);

    // Launch on click
    card.addEventListener('click', async () => {
        try {
            await invoke('launch_application', { exec: app.exec });
        } catch (err) {
            console.error('Failed to launch:', err);
        }
    });

    return card;
}

// Render apps to grid
function renderApps(apps) {
    appGrid.innerHTML = '';

    if (apps.length === 0) {
        appGrid.innerHTML = `
      <div class="empty-state">
        <div class="icon">🔍</div>
        <div class="message">No applications found</div>
      </div>
    `;
        return;
    }

    apps.forEach(app => {
        appGrid.appendChild(createAppCard(app));
    });
}

// Search handler
const handleSearch = debounce(async (query) => {
    try {
        const apps = await invoke('search_applications', { query });
        renderApps(apps);
    } catch (err) {
        console.error('Search failed:', err);
    }
}, 150);

// Initialize
async function init() {
    try {
        // Load all applications
        const apps = await invoke('get_applications');
        renderApps(apps);

        // Setup search
        searchInput.addEventListener('input', (e) => {
            handleSearch(e.target.value);
        });

        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            // Focus search on Ctrl+F or /
            if ((e.ctrlKey && e.key === 'f') || (e.key === '/' && document.activeElement !== searchInput)) {
                e.preventDefault();
                searchInput.focus();
                searchInput.select();
            }

            // Clear search on Escape
            if (e.key === 'Escape') {
                if (searchInput.value) {
                    searchInput.value = '';
                    handleSearch('');
                } else {
                    // Typically a launcher would hide here, but we'll just blur for now
                    searchInput.blur();
                }
            }

            // Arrow Navigation
            if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
                e.preventDefault();
                const cards = Array.from(document.querySelectorAll('.app-card'));
                if (cards.length === 0) return;

                const current = document.activeElement.classList.contains('app-card') ? document.activeElement : null;
                let nextIndex = 0;

                if (current) {
                    const index = cards.indexOf(current);
                    if (e.key === 'ArrowRight') nextIndex = index + 1;
                    if (e.key === 'ArrowLeft') nextIndex = index - 1;
                    if (e.key === 'ArrowDown') nextIndex = index + 4; // grid width approx
                    if (e.key === 'ArrowUp') nextIndex = index - 4;
                }

                // Clamp index
                if (nextIndex < 0) nextIndex = 0;
                if (nextIndex >= cards.length) nextIndex = cards.length - 1;

                cards[nextIndex].focus();
                cards[nextIndex].scrollIntoView({ behavior: 'smooth', block: 'nearest' });
            }

            // Launch on Enter
            if (e.key === 'Enter') {
                const current = document.activeElement;
                if (current.classList.contains('app-card')) {
                    current.click();
                } else if (document.activeElement === searchInput) {
                    // Launch top result
                    const firstCard = document.querySelector('.app-card');
                    if (firstCard) firstCard.click();
                }
            }
        });

    } catch (err) {
        console.error('Failed to load applications:', err);
        appGrid.innerHTML = `
      <div class="empty-state">
        <div class="icon">⚠️</div>
        <div class="message">Failed to load applications</div>
      </div>
    `;
    }
}

// Start when DOM is ready
document.addEventListener('DOMContentLoaded', init);
