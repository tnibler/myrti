import PhotoSwipe from 'photoswipe';
import PhotoSwipeLightbox from '../../node_modules/photoswipe/dist/photoswipe-lightbox.esm';
import 'photoswipe/dist/photoswipe.css';

const images = [
  { id: 1, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+1', width: 1500, height: 1000 },
  { id: 2, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+2', width: 1500, height: 1000 },
  { id: 3, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+3', width: 1500, height: 1000 },
  { id: 4, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+4', width: 1500, height: 1000 },
  { id: 5, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+5', width: 1500, height: 1000 },
  { id: 6, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+6', width: 1500, height: 1000 },
  { id: 7, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+7', width: 1500, height: 1000 },
  { id: 8, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+8', width: 1500, height: 1000 },
  { id: 9, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+9', width: 1500, height: 1000 },
  { id: 10, src: 'https://dummyimage.com/1500x1000/555/fff/?text=Image+10', width: 1500, height: 1000 },
];

export function photoswipe(node) {
  const lightbox: PhotoSwipeLightbox = new PhotoSwipeLightbox({
    // showHideAnimationType: 'none',
    pswpModule: PhotoSwipe,
    preload: [1, 2],
  });
  lightbox.addFilter('numItems', (numItems) => {
    return 1000;
  });
  lightbox.addFilter('itemData', (itemData, index) => {
    return {
      src: 'https://dummyimage.com/100x100/555/fff/?text=' + (index + 1),
      width: 100,
      height: 100
    };;
  });
  lightbox.init();
  lightbox.ui?.init();


  function show() {
    lightbox.loadAndOpen(2);
  }
  node.addEventListener("click", show);

  return {
    destroy() {
      lightbox.destroy();
    }
  }
}

