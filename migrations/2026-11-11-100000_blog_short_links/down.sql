DROP TRIGGER preserve_blog_publication_link ON blog_publication_links;
DROP FUNCTION preserve_blog_publication_link();
DROP TRIGGER assign_blog_publication_link ON posts;
DROP FUNCTION assign_blog_publication_link();
DROP FUNCTION allocate_blog_publication_link(uuid);
DROP TABLE blog_publication_links;
